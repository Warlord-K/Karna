use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use tracing::{info, warn};

use crate::cli::{self, CliOptions};
use crate::config::Config;
use crate::db::Database;
use crate::git::workspace;
use crate::memory::{
    agent_namespace, build_memory_section, dedupe_snippets, profile_slug, repo_namespace,
    user_namespace, MemoryClient, MemorySnippet,
};
use crate::models::{AgentTask, OrchestratorConfig, TaskKind, TaskOutputTarget, TaskStatus};

use super::actions;

/// Run a non-code task in a scratch directory, with MCP enabled and no git
/// worktree/commit/PR workflow.
pub async fn run_generic(config: &Config, db: &Database, task: &AgentTask) -> Result<()> {
    info!(task_id = %task.id, kind = %task.kind, "Starting generic non-code flow");
    if task.kind_enum() == TaskKind::Ops {
        if let Some(orchestrator) = task.orchestrator_config() {
            return run_orchestrator(config, db, task, &orchestrator).await;
        }
    }
    run_standard_generic(config, db, task).await
}

async fn run_standard_generic(config: &Config, db: &Database, task: &AgentTask) -> Result<()> {
    db.update_status(task.id, TaskStatus::InProgress.as_str())
        .await?;
    db.insert_log(
        task.id,
        "generic",
        &format!("Starting non-code flow for: {}", task.title),
        "info",
        None,
    )
    .await?;

    let scratch_dir = config.workspaces_dir.join(task.id.to_string());
    tokio::fs::create_dir_all(&scratch_dir).await?;
    db.insert_log(
        task.id,
        "generic",
        &format!("Using scratch workspace: {}", scratch_dir.display()),
        "info",
        None,
    )
    .await?;

    let output_target = task.output_target_enum();
    let mut prompt = build_generic_prompt(task, output_target);
    let (cli_name, model, addendum) =
        super::resolve_runtime(db, task, config, super::Stage::Implement).await;
    let system_prompt =
        super::merge_system_prompt(config.instructions.as_deref(), addendum.as_deref());
    inject_memory_section(config, db, task, "generic", &mut prompt, &cli_name, &model).await?;

    db.insert_log(
        task.id,
        "generic",
        &format!("Invoking {cli_name} ({model}) for non-code flow"),
        "command",
        None,
    )
    .await?;

    let event_tx = super::spawn_log_consumer(db.clone(), task.id, "generic");
    let result = cli::run(
        &cli_name,
        CliOptions {
            working_dir: &scratch_dir,
            prompt: &prompt,
            system_prompt: system_prompt.as_deref(),
            allowed_tools: Some("Read,Glob,Grep,Bash"),
            max_turns: config.max_turns,
            model: &model,
            mcp_config_json: config.mcp_config_json().map(|v| v.to_string()),
            session_id: None,
            resume: false,
            event_tx: Some(event_tx),
            image_paths: Vec::new(),
        },
    )
    .await?;

    db.add_cost(task.id, result.cost_usd).await?;
    if let Some(sid) = &result.session_id {
        db.set_session_id(task.id, sid).await?;
    }

    let (artifact, output_ref_from_output) = extract_artifact_and_ref(&result.output);
    let output_ref = write_output_target(
        config,
        db,
        task,
        output_target,
        &artifact,
        output_ref_from_output,
    )
    .await?;

    if let Some(output_ref) = output_ref.clone() {
        let mut updates = HashMap::new();
        updates.insert("output_ref".to_string(), Value::String(output_ref));
        let _ = db.update_task(task.id, task.user_id, &updates).await?;
    }
    db.update_status(task.id, TaskStatus::Done.as_str()).await?;

    if matches!(
        output_target,
        TaskOutputTarget::Notification | TaskOutputTarget::None
    ) {
        let _ = crate::notifications::send_non_code_output(
            config,
            db,
            task,
            &artifact,
            output_ref.as_deref(),
        )
        .await;
    }

    db.insert_log(task.id, "generic", "Non-code flow completed", "info", None)
        .await?;
    Ok(())
}

async fn run_orchestrator(
    config: &Config,
    db: &Database,
    task: &AgentTask,
    orchestrator: &OrchestratorConfig,
) -> Result<()> {
    db.update_status(task.id, TaskStatus::InProgress.as_str())
        .await?;
    db.set_not_before(task.id, None).await?;

    let turns_taken = db
        .count_logs_for_phase(task.id, "orchestrator_turn")
        .await?;
    if turns_taken >= i64::from(orchestrator.max_turns) {
        return actions::escalate_and_close(
            config,
            db,
            task,
            &format!(
                "max_turns exceeded ({} >= {})",
                turns_taken, orchestrator.max_turns
            ),
        )
        .await;
    }

    if let Some(deadline) = orchestrator.deadline.as_deref() {
        let now = Utc::now();
        let deadline_at = actions::resolve_deadline(deadline, task.created_at, now)?;
        if now > deadline_at {
            return actions::escalate_and_close(
                config,
                db,
                task,
                &format!("deadline exceeded at {}", deadline_at.to_rfc3339()),
            )
            .await;
        }
    }

    let turn_number = turns_taken + 1;
    db.insert_log(
        task.id,
        "orchestrator_turn",
        &format!("Starting orchestrator turn #{turn_number}"),
        "info",
        None,
    )
    .await?;

    let working_dir = if let Some(repo_ref) = task.repos().first() {
        let repo_config = config.find_repo(repo_ref);
        let repo_url = repo_config.map(|r| r.repo.as_str()).unwrap_or(repo_ref);
        let base_branch = repo_config.map(|r| r.branch.as_str()).unwrap_or("main");

        let repo_path =
            workspace::ensure_cloned(&config.repos_dir, repo_url, &config.github_token).await?;
        workspace::checkout_and_pull(&repo_path, base_branch).await?;
        db.insert_log(
            task.id,
            "orchestrator",
            &format!(
                "Using read-only repo workspace: {} @ {}",
                repo_path.display(),
                base_branch
            ),
            "info",
            None,
        )
        .await?;
        repo_path
    } else {
        let scratch_dir = config.workspaces_dir.join(task.id.to_string());
        tokio::fs::create_dir_all(&scratch_dir).await?;
        scratch_dir
    };

    let available_mcp_tools = if config.mcp_servers.is_empty() {
        "(no global MCP servers configured)".to_string()
    } else {
        config
            .mcp_servers
            .iter()
            .map(|server| format!("- {}", server.name))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let allowed_action_tools = if orchestrator.allowed_tools.is_empty() {
        "(none)".to_string()
    } else {
        orchestrator
            .allowed_tools
            .iter()
            .map(|tool| format!("- {tool}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let description = task
        .description
        .as_deref()
        .unwrap_or("No description provided.");
    let latest_message = task
        .feedback
        .as_deref()
        .unwrap_or("No new thread message for this turn.");
    let task_context = task
        .plan_content
        .as_deref()
        .unwrap_or("No prior plan or scratch context.");
    let constraints_json =
        serde_json::to_string_pretty(orchestrator).unwrap_or_else(|_| "{}".to_string());

    let mut prompt = include_str!("../../templates/orchestrator_prompt.txt").to_string();
    prompt = prompt.replace("{title}", &task.title);
    prompt = prompt.replace("{description}", description);
    prompt = prompt.replace("{latest_message}", latest_message);
    prompt = prompt.replace("{task_context}", task_context);
    prompt = prompt.replace("{constraints_json}", &constraints_json);
    prompt = prompt.replace("{allowed_action_tools}", &allowed_action_tools);
    prompt = prompt.replace("{available_mcp_tools}", &available_mcp_tools);

    let (cli_name, model, addendum) =
        super::resolve_runtime(db, task, config, super::Stage::Implement).await;
    let system_prompt =
        super::merge_system_prompt(config.instructions.as_deref(), addendum.as_deref());
    inject_memory_section(
        config,
        db,
        task,
        "orchestrator",
        &mut prompt,
        &cli_name,
        &model,
    )
    .await?;

    let event_tx = super::spawn_log_consumer(db.clone(), task.id, "orchestrator");
    let has_session = task.agent_session_id.is_some();
    let result = cli::run(
        &cli_name,
        CliOptions {
            working_dir: &working_dir,
            prompt: &prompt,
            system_prompt: system_prompt.as_deref(),
            allowed_tools: Some("Read,Glob,Grep,Bash"),
            max_turns: config.max_turns,
            model: &model,
            mcp_config_json: config.mcp_config_json().map(|v| v.to_string()),
            session_id: task.agent_session_id.as_deref(),
            resume: has_session,
            event_tx: Some(event_tx),
            image_paths: Vec::new(),
        },
    )
    .await?;

    db.add_cost(task.id, result.cost_usd).await?;
    if let Some(sid) = &result.session_id {
        db.set_session_id(task.id, sid).await?;
    }

    let actions = match actions::parse_actions_from_output(&result.output) {
        Ok(parsed) => parsed,
        Err(error) => {
            db.insert_log(
                task.id,
                "orchestrator",
                &format!("Invalid actions block: {error}"),
                "warning",
                None,
            )
            .await?;
            db.set_feedback(
                task.id,
                &format!(
                    "Previous turn failed to emit a valid <!-- actions --> block ({error}). Emit only valid v1 actions."
                ),
            )
            .await?;
            return Ok(());
        }
    };

    let execution = match actions::execute_actions(config, db, task, orchestrator, &actions).await {
        Ok(done) => done,
        Err(error) => {
            db.insert_log(
                task.id,
                "orchestrator",
                &format!("Action execution blocked: {error}"),
                "warning",
                None,
            )
            .await?;
            db.set_feedback(
                task.id,
                &format!(
                    "Previous action block violated guardrails ({error}). Emit a compliant <!-- actions --> block."
                ),
            )
            .await?;
            return Ok(());
        }
    };

    if !execution.wrote_feedback {
        let original_feedback = task.feedback.as_deref().unwrap_or("");
        if let Some(current) = db.get_task(task.id).await? {
            let current_feedback = current.feedback.as_deref().unwrap_or("");
            if current_feedback == original_feedback {
                db.clear_feedback(task.id).await?;
            }
        }
    }

    if execution.closed {
        db.insert_log(
            task.id,
            "orchestrator",
            &format!("Closed on turn #{turn_number}"),
            "info",
            None,
        )
        .await?;
        return Ok(());
    }

    if let Some(until) = execution.deferred_until {
        db.insert_log(
            task.id,
            "orchestrator",
            &format!("Deferred until {}", until.to_rfc3339()),
            "info",
            None,
        )
        .await?;
    } else {
        db.insert_log(
            task.id,
            "orchestrator",
            "Waiting for next thread reply or queued follow-up turn",
            "info",
            None,
        )
        .await?;
    }

    Ok(())
}

fn build_generic_prompt(task: &AgentTask, output_target: TaskOutputTarget) -> String {
    let description = task
        .description
        .as_deref()
        .unwrap_or("No description provided.");
    let plan = task.plan_content.as_deref().unwrap_or("No plan provided.");
    let external_context = match (
        task.external_source.as_deref(),
        task.external_id.as_deref(),
        task.external_url.as_deref(),
    ) {
        (Some(source), Some(id), Some(url)) => {
            format!("- source: {source}\n- id: {id}\n- url: {url}")
        }
        (Some(source), Some(id), None) => format!("- source: {source}\n- id: {id}"),
        _ => "None".to_string(),
    };
    format!(
        r#"You are executing a non-code task for Karna.

## Task
- title: {title}
- kind: {kind}
- output_target: {output_target}

## Description
{description}

## Plan (optional context)
{plan}

## External Context
{external_context}

## Execution Rules
- Do not use git worktrees, git commits, or pull requests.
- Use available MCP tools when needed to complete the objective.
- If output_target is `linear_comment`, `linear_doc`, or `slack_message`, publish the artifact to that destination during this run.
- Always produce a polished markdown artifact.

## Final Response Contract
End your final message with these exact sections:

===ARTIFACT===
<final markdown artifact text>

===OUTPUT_REF===
<URL or identifier where the artifact was delivered, or `none` if not applicable>
"#,
        title = task.title,
        kind = task.kind,
        output_target = output_target.as_str(),
        description = description,
        plan = plan,
        external_context = external_context,
    )
}

async fn inject_memory_section(
    config: &Config,
    db: &Database,
    task: &AgentTask,
    phase: &str,
    prompt: &mut String,
    cli_name: &str,
    model: &str,
) -> Result<()> {
    let client = MemoryClient::new(&config.memory);
    let mut injected = 0usize;
    if client.is_enabled() {
        let mut namespaces: Vec<String> = task
            .repos()
            .iter()
            .map(|repo| repo_namespace(repo))
            .collect();
        namespaces.push(agent_namespace(&profile_slug(cli_name, model)));
        namespaces.push(user_namespace(task.user_id));
        namespaces.sort();
        namespaces.dedup();

        let mut snippets = Vec::new();
        let memory_query = format!(
            "{title}\n\n{description}",
            title = task.title,
            description = task.description.as_deref().unwrap_or(""),
        );
        for namespace in namespaces {
            let items = client
                .search(&memory_query, &namespace, config.memory.max_items)
                .await;
            for item in items {
                snippets.push(MemorySnippet {
                    namespace: namespace.clone(),
                    text: item.text,
                });
            }
        }

        let snippets = dedupe_snippets(snippets);
        if let Some(section) =
            build_memory_section(&snippets, config.memory.max_items, config.memory.max_chars)
        {
            prompt.push_str("\n\n");
            prompt.push_str(&section.text);
            injected = section.item_count;
        }
    }
    db.insert_log(
        task.id,
        phase,
        &format!("Injected {injected} memories into prompt"),
        "info",
        None,
    )
    .await?;
    Ok(())
}

async fn write_output_target(
    config: &Config,
    db: &Database,
    task: &AgentTask,
    output_target: TaskOutputTarget,
    artifact: &str,
    output_ref_from_output: Option<String>,
) -> Result<Option<String>> {
    let mut output_ref = output_ref_from_output;
    match output_target {
        TaskOutputTarget::Notification | TaskOutputTarget::None => {
            db.insert_log(task.id, "artifact", artifact, "output", None)
                .await?;
        }
        TaskOutputTarget::SlackMessage => {
            db.insert_log(task.id, "artifact", artifact, "output", None)
                .await?;
            let slack_ref = crate::slack::send_task_message(config, db, task, artifact).await?;
            if output_ref.is_none() {
                output_ref = slack_ref;
            }
        }
        TaskOutputTarget::LinearComment | TaskOutputTarget::LinearDoc => {
            db.insert_log(task.id, "artifact", artifact, "output", None)
                .await?;
            if output_ref.is_none() {
                warn!(
                    task_id = %task.id,
                    target = output_target.as_str(),
                    "No OUTPUT_REF returned by generic run for Linear target"
                );
            }
        }
        TaskOutputTarget::Pr => {
            db.insert_log(
                task.id,
                "artifact",
                "Output target `pr` is unsupported for non-code flow; kept artifact in task logs",
                "warning",
                None,
            )
            .await?;
            db.insert_log(task.id, "artifact", artifact, "output", None)
                .await?;
        }
    }
    Ok(output_ref)
}

fn extract_artifact_and_ref(output: &str) -> (String, Option<String>) {
    let artifact = extract_section(output, "ARTIFACT").unwrap_or_else(|| output.trim().to_string());
    let output_ref = extract_section(output, "OUTPUT_REF")
        .and_then(|s| normalize_output_ref(&s))
        .or_else(|| first_url(output));
    (artifact, output_ref)
}

fn extract_section(output: &str, section: &str) -> Option<String> {
    let marker = format!("==={section}===");
    let start = output.find(&marker)?;
    let rest = output[start + marker.len()..].trim_start();
    let end = rest.find("\n===").unwrap_or(rest.len());
    let section_text = rest[..end].trim();
    if section_text.is_empty() {
        None
    } else {
        Some(section_text.to_string())
    }
}

fn normalize_output_ref(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn first_url(output: &str) -> Option<String> {
    for token in output.split_whitespace() {
        let cleaned = token
            .trim_matches(|c: char| ",.;:!?)('\"[]{}".contains(c))
            .to_string();
        if cleaned.starts_with("http://") || cleaned.starts_with("https://") {
            return Some(cleaned);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{extract_artifact_and_ref, extract_section, normalize_output_ref};

    #[test]
    fn extracts_delimited_sections() {
        let output = "Intro\n===ARTIFACT===\nHello world\n===OUTPUT_REF===\nhttps://example.com/x";
        let (artifact, output_ref) = extract_artifact_and_ref(output);
        assert_eq!(artifact, "Hello world");
        assert_eq!(output_ref.as_deref(), Some("https://example.com/x"));
    }

    #[test]
    fn falls_back_to_full_output_without_sections() {
        let output = "Plain final response";
        let (artifact, output_ref) = extract_artifact_and_ref(output);
        assert_eq!(artifact, "Plain final response");
        assert!(output_ref.is_none());
    }

    #[test]
    fn normalizes_none_output_ref() {
        assert!(normalize_output_ref("none").is_none());
        assert!(normalize_output_ref("   ").is_none());
        assert_eq!(
            normalize_output_ref("https://example.com").as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn extracts_named_section() {
        let output = "===ARTIFACT===\nA\n===OUTPUT_REF===\nnone";
        assert_eq!(extract_section(output, "ARTIFACT").as_deref(), Some("A"));
    }
}
