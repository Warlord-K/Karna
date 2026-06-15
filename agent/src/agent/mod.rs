mod actions;
mod flow;
mod implementer;
pub(crate) mod planner;
mod self_review;

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::cli::{truncate_for_log, EventSender, StreamEvent, TOOL_OUTPUT_MAX_CHARS};
use crate::config::Config;
use crate::db::Database;
use crate::git::workspace;
use crate::memory::{
    agent_namespace, profile_slug, repo_namespace, summarize_task_for_memory, user_namespace,
    AddPayload, MemoryClient, MemoryMessage,
};
use crate::models::{AgentTask, TaskKind, TaskStatus};
use crate::queue;

/// A stage in the multi-agent flow. Each stage can resolve to a different
/// tool/model via the task's per-stage agent profile columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Scope / planning.
    Plan,
    /// Implementation (and feedback application).
    Implement,
    /// Self-review of the working-tree diff.
    Review,
}

impl Stage {
    /// The per-stage agent profile column for this stage.
    fn agent_id(self, task: &AgentTask) -> Option<uuid::Uuid> {
        match self {
            Stage::Plan => task.planner_agent_id,
            Stage::Implement => task.implementer_agent_id,
            Stage::Review => task.reviewer_agent_id,
        }
    }
}

/// Resolve which CLI + model + agent-specific system prompt to use for a stage
/// of a task.
///
/// Precedence:
///   1. the stage's agent profile (`planner_/implementer_/reviewer_agent_id`) —
///      when set, it fully owns cli + model for this stage
///   2. task.cli / task.model — explicit per-task override (legacy)
///   3. task.assigned_agent_id profile — task-level agent
///   4. config defaults
///
/// The third element is the resolved profile's `system_prompt_addendum`,
/// appended to the global instructions file (if any) at CLI invocation time.
pub async fn resolve_runtime(
    db: &Database,
    task: &AgentTask,
    config: &Config,
    stage: Stage,
) -> (String, String, Option<String>) {
    // Stage-specific profile takes top precedence when present.
    let stage_profile = match stage.agent_id(task) {
        Some(id) => db.get_agent_profile(id).await.ok().flatten(),
        None => None,
    };

    // Fall back to the task-level assigned agent only when no stage agent set.
    let fallback_profile = if stage_profile.is_none() {
        match task.assigned_agent_id {
            Some(id) => db.get_agent_profile(id).await.ok().flatten(),
            None => None,
        }
    } else {
        None
    };

    let (cli, model) = if let Some(p) = &stage_profile {
        (p.cli.clone(), p.model.clone())
    } else {
        let cli = task
            .cli
            .clone()
            .or_else(|| fallback_profile.as_ref().map(|p| p.cli.clone()))
            .unwrap_or_else(|| config.default_cli().to_string());
        let model = task
            .model
            .clone()
            .or_else(|| fallback_profile.as_ref().map(|p| p.model.clone()))
            .unwrap_or_else(|| config.default_model(&cli).to_string());
        (cli, model)
    };

    let addendum = stage_profile
        .or(fallback_profile)
        .and_then(|p| p.system_prompt_addendum);

    (cli, model, addendum)
}

/// Merge the global instructions file with an agent profile's system prompt
/// addendum, returning whatever should be passed as `CliOptions.system_prompt`.
pub fn merge_system_prompt(global: Option<&str>, addendum: Option<&str>) -> Option<String> {
    match (global, addendum) {
        (Some(g), Some(a)) => Some(format!("{g}\n\n{a}")),
        (Some(g), None) => Some(g.to_string()),
        (None, Some(a)) => Some(a.to_string()),
        (None, None) => None,
    }
}

/// Spawn a background task that consumes CLI stream events and inserts them as agent logs.
/// Returns the sender half — pass it to `CliOptions.event_tx`.
pub fn spawn_log_consumer(db: Database, task_id: Uuid, phase: &'static str) -> EventSender {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    tokio::spawn(async move {
        let mut pending_inputs: HashMap<String, VecDeque<String>> = HashMap::new();
        while let Some(event) = rx.recv().await {
            let (message, log_type, metadata) = match event {
                StreamEvent::ToolUse {
                    tool,
                    input_summary,
                } => {
                    let mut metadata = json!({ "tool": tool });
                    if !input_summary.is_empty() {
                        if let Some(obj) = metadata.as_object_mut() {
                            obj.insert("input".to_string(), Value::String(input_summary.clone()));
                        }
                        pending_inputs
                            .entry(tool.clone())
                            .or_default()
                            .push_back(input_summary.clone());
                    }
                    let msg = if input_summary.is_empty() {
                        tool.clone()
                    } else {
                        format!("{tool}: {input_summary}")
                    };
                    (msg, "tool", Some(metadata))
                }
                StreamEvent::ToolResult { tool, output } => {
                    let output = truncate_for_log(output.trim(), TOOL_OUTPUT_MAX_CHARS);
                    if output.is_empty() {
                        continue;
                    }
                    let mut metadata = json!({
                        "tool": tool,
                        "output": output,
                    });
                    if let Some(input) = pending_inputs
                        .get_mut(&tool)
                        .and_then(|queue| queue.pop_front())
                    {
                        if let Some(obj) = metadata.as_object_mut() {
                            obj.insert("input".to_string(), Value::String(input));
                        }
                    }
                    (format!("{tool} output"), "tool", Some(metadata))
                }
                StreamEvent::AssistantText(text) => {
                    // Only log substantial text
                    let trimmed = text.trim();
                    if trimmed.len() < 20 {
                        continue;
                    }
                    let truncated: String = trimmed.chars().take(300).collect();
                    (truncated, "output", None)
                }
                StreamEvent::Error(e) => (format!("Error: {e}"), "error", None),
            };
            let _ = db
                .insert_log(task_id, phase, &message, log_type, metadata)
                .await;
        }
    });
    tx
}

/// Single poll iteration. Called every N seconds from main loop.
pub async fn poll_once(config: &Config, db: &Database, redis: &redis::Client) -> Result<()> {
    let worker = queue::worker_id();

    // 0. Cleanup stale worktrees from completed/failed tasks (older than 24h)
    if let Ok(stale_tasks) = db.stale_completed_tasks(24).await {
        for task in stale_tasks {
            let task_dir = config.workspaces_dir.join(task.id.to_string());
            if task_dir.exists() {
                debug!(task_id = %task.id, "Cleaning up worktree for completed task");
                // Remove worktrees for each repo
                for repo_ref in task.repos() {
                    let repo_name = repo_ref.rsplit('/').next().unwrap_or(repo_ref);
                    let worktree_path = task_dir.join(repo_name);
                    let repo_path = config.repos_dir.join(repo_name);
                    let _ = workspace::remove_worktree(&repo_path, &worktree_path).await;
                }
                let _ = tokio::fs::remove_dir_all(&task_dir).await;
            }
        }
    }

    // 0.5 Write back concise memory summaries for tasks that recently reached done.
    if MemoryClient::new(&config.memory).is_enabled() {
        match db.done_tasks_pending_memory_writeback(10).await {
            Ok(tasks) => {
                for task in tasks {
                    write_back_task_memory(config, db, &task).await;
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to load done tasks for memory write-back");
            }
        }
    }

    // 1. Check for tasks in review/plan_review with pending feedback
    //    review → in_progress (apply feedback to PR)
    //    plan_review → planning (re-plan with feedback)
    //    (skip parent tasks — they don't have PRs, subtasks do)
    let feedback_tasks = db.tasks_with_pending_feedback().await?;
    for task in feedback_tasks {
        if task.repo.is_none() {
            let subtasks = db.get_subtasks(task.id).await?;
            if !subtasks.is_empty() {
                continue;
            }
        }
        let next_status = if task.status == TaskStatus::PlanReview.as_str() {
            TaskStatus::Planning
        } else {
            TaskStatus::InProgress
        };
        info!(task_id = %task.id, status = ?task.status, "Task has pending feedback, moving to {}", next_status.as_str());
        db.update_status(task.id, next_status.as_str()).await?;
    }

    // 2. Check if any task is genuinely locked by a worker (not just in an active DB status).
    //    A task can be in_progress without a lock if the user approved a plan.
    let active_ids = db.active_task_ids().await?;
    for id in active_ids {
        if queue::is_locked(redis, id).await? {
            return Ok(()); // A worker is actively processing a task
        }
    }

    // 3. Find the next actionable task
    let task = match db.next_actionable_task().await? {
        Some(t) => t,
        None => return Ok(()), // Nothing to do
    };

    let status = match task.status_enum() {
        Some(s) => s,
        None => {
            warn!(status = %task.status, "Unknown task status");
            return Ok(());
        }
    };

    // 4. Try to acquire lock
    if !queue::try_lock(redis, task.id, &worker).await? {
        info!(task_id = %task.id, "Task locked by another worker, skipping");
        return Ok(());
    }

    info!(task_id = %task.id, title = %task.title, status = %task.status, "Claimed task");

    // 5. Dispatch based on kind + status.
    let result = if should_run_generic_flow(&task) {
        match status {
            TaskStatus::Todo | TaskStatus::Planning | TaskStatus::InProgress => {
                run_with_heartbeat(redis, task.id, async {
                    flow::run_generic(config, db, &task).await
                })
                .await
            }
            _ => Ok(()),
        }
    } else {
        match status {
            TaskStatus::Todo | TaskStatus::Planning => {
                run_with_heartbeat(redis, task.id, async {
                    planner::plan_task(config, db, &task).await
                })
                .await
            }
            TaskStatus::InProgress => {
                if task.pr_url.is_some() {
                    // Has a PR already — this is a feedback cycle
                    run_with_heartbeat(redis, task.id, async {
                        implementer::apply_feedback(config, db, &task).await
                    })
                    .await
                } else {
                    // Fresh implementation after plan approval
                    run_with_heartbeat(redis, task.id, async {
                        implementer::implement_task(config, db, &task).await
                    })
                    .await
                }
            }
            _ => Ok(()),
        }
    };

    // 6. Handle errors
    if let Err(e) = result {
        error!(task_id = %task.id, error = %e, "Task failed");
        let error_msg = format!("{e:#}");
        db.set_error(task.id, &error_msg).await?;
        db.insert_log(task.id, "error", &error_msg, "error", None)
            .await?;
        let _ = crate::notifications::send_task_failed(config, db, &task).await;
    }

    // 7. Release lock
    queue::release(redis, task.id).await?;

    Ok(())
}

async fn write_back_task_memory(config: &Config, db: &Database, task: &AgentTask) {
    let Some(summary) = summarize_task_for_memory(task) else {
        let _ = db
            .insert_log(
                task.id,
                "memory",
                "Memory write-back complete",
                "info",
                None,
            )
            .await;
        return;
    };

    let mut namespaces: Vec<String> = task
        .repos()
        .iter()
        .map(|repo| repo_namespace(repo))
        .collect();
    namespaces.push(agent_namespace(
        &done_task_profile_slug(config, db, task).await,
    ));
    namespaces.push(user_namespace(task.user_id));
    namespaces.sort();
    namespaces.dedup();

    let client = MemoryClient::new(&config.memory);
    let payload = if summary.len() > 400 {
        AddPayload::Text(summary.clone())
    } else {
        AddPayload::Messages(vec![MemoryMessage {
            role: "user".to_string(),
            content: summary.clone(),
        }])
    };
    let mut targets = 0usize;
    for namespace in namespaces {
        client.add(payload.clone(), &namespace).await;
        targets += 1;
    }

    let _ = db
        .insert_log(
            task.id,
            "memory",
            &format!("Saved summary to {targets} memory namespace(s)"),
            "info",
            None,
        )
        .await;
    let _ = db
        .insert_log(
            task.id,
            "memory",
            "Memory write-back complete",
            "info",
            None,
        )
        .await;
}

async fn done_task_profile_slug(config: &Config, db: &Database, task: &AgentTask) -> String {
    if let Some(profile_id) = task.implementer_agent_id.or(task.assigned_agent_id) {
        if let Ok(Some(profile)) = db.get_agent_profile(profile_id).await {
            return profile.slug;
        }
    }

    let cli = task
        .cli
        .clone()
        .unwrap_or_else(|| config.default_cli().to_string());
    let model = task
        .model
        .clone()
        .unwrap_or_else(|| config.default_model(&cli).to_string());
    profile_slug(&cli, &model)
}

fn should_run_generic_flow(task: &AgentTask) -> bool {
    task.kind_enum() != TaskKind::Code
}

/// Run a future with periodic Redis heartbeats to keep the lock alive.
async fn run_with_heartbeat<F, T>(redis: &redis::Client, task_id: Uuid, work: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    let redis_clone = redis.clone();
    let heartbeat_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            if let Err(e) = queue::heartbeat(&redis_clone, task_id).await {
                warn!(error = %e, "Heartbeat failed");
                break;
            }
        }
    });

    let result = work.await;

    heartbeat_handle.abort();
    result
}

#[cfg(test)]
mod tests {
    use super::should_run_generic_flow;
    use crate::models::AgentTask;
    use uuid::Uuid;

    #[test]
    fn runs_generic_flow_for_non_code_kinds() {
        let mut task = fixture_task();
        task.kind = "doc".to_string();
        assert!(should_run_generic_flow(&task));

        task.kind = "research".to_string();
        assert!(should_run_generic_flow(&task));

        task.kind = "ops".to_string();
        assert!(should_run_generic_flow(&task));
    }

    #[test]
    fn keeps_code_flow_for_code_or_unknown_kind() {
        let mut task = fixture_task();
        task.kind = "code".to_string();
        assert!(!should_run_generic_flow(&task));

        task.kind = "something_else".to_string();
        assert!(!should_run_generic_flow(&task));
    }

    fn fixture_task() -> AgentTask {
        AgentTask {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            assignee_user_id: None,
            title: "Example task".to_string(),
            description: None,
            repo: None,
            kind: "code".to_string(),
            output_target: Some("none".to_string()),
            output_ref: None,
            source: None,
            parent_task_id: None,
            target_branch: None,
            status: "todo".to_string(),
            priority: "medium".to_string(),
            position: 0.0,
            branch: None,
            pr_url: None,
            pr_number: None,
            plan_content: None,
            result_content: None,
            feedback: None,
            not_before: None,
            agent_session_id: None,
            error_message: None,
            cli: None,
            model: None,
            task_number: Some(1),
            cost_usd: 0.0,
            external_source: None,
            external_id: None,
            external_url: None,
            slack_channel: None,
            slack_thread_ts: None,
            assigned_agent_id: None,
            planner_agent_id: None,
            implementer_agent_id: None,
            reviewer_agent_id: None,
            orchestrator: None,
            policy_matches: None,
            created_at: None,
            updated_at: None,
            started_at: None,
            completed_at: None,
        }
    }
}
