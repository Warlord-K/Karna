pub(crate) mod planner;
mod implementer;

use anyhow::Result;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::cli::{EventSender, StreamEvent};
use crate::config::Config;
use crate::db::Database;
use crate::git::workspace;
use crate::models::TaskStatus;
use crate::queue;

/// Spawn a background task that consumes CLI stream events and inserts them as agent logs.
/// Returns the sender half — pass it to `CliOptions.event_tx`.
pub fn spawn_log_consumer(db: Database, task_id: Uuid, phase: &'static str) -> EventSender {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let (message, log_type) = match event {
                StreamEvent::ToolUse {
                    tool,
                    input_summary,
                } => {
                    let msg = if input_summary.is_empty() {
                        tool.clone()
                    } else {
                        format!("{tool}: {input_summary}")
                    };
                    (msg, "tool")
                }
                StreamEvent::AssistantText(text) => {
                    // Only log substantial text
                    let trimmed = text.trim();
                    if trimmed.len() < 20 {
                        continue;
                    }
                    let truncated: String = trimmed.chars().take(300).collect();
                    (truncated, "output")
                }
                StreamEvent::Error(e) => (format!("Error: {e}"), "error"),
            };
            let _ = db
                .insert_log(task_id, phase, &message, log_type, None)
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

    // 5. Dispatch based on status
    let result = match status {
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
    };

    // 6. Handle errors
    if let Err(e) = result {
        error!(task_id = %task.id, error = %e, "Task failed");
        let error_msg = format!("{e:#}");
        db.set_error(task.id, &error_msg).await?;
        db.insert_log(task.id, "error", &error_msg, "error", None).await?;
        let _ = crate::notifications::send_task_failed(config, &task).await;
    }

    // 7. Release lock
    queue::release(redis, task.id).await?;

    Ok(())
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
