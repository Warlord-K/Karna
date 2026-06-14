use std::fmt::Write as _;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use regex_lite::Regex;
use serde::Deserialize;
use tracing::warn;

use crate::config::Config;
use crate::db::Database;
use crate::models::{AgentTask, OrchestratorConfig, TaskKind, TaskStatus};

const ACTIONS_BLOCK_PATTERN: &str = r"<!--\s*actions\s*\n([\s\S]*?)\nactions\s*-->";

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorAction {
    Reply {
        text: String,
    },
    Run {
        tool: String,
        #[serde(default)]
        args: serde_json::Value,
    },
    Defer {
        #[serde(rename = "in")]
        in_for: String,
        #[serde(default)]
        note: Option<String>,
    },
    Subtask {
        title: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        repo: Option<String>,
    },
    Escalate {
        text: String,
        #[serde(default)]
        channel: Option<String>,
    },
    Close {
        #[serde(default)]
        reason: Option<String>,
    },
}

#[derive(Debug, Default)]
pub struct ActionExecutionOutcome {
    pub closed: bool,
    pub deferred_until: Option<DateTime<Utc>>,
    pub wrote_feedback: bool,
}

pub fn parse_actions_from_output(output: &str) -> Result<Vec<OrchestratorAction>> {
    let re = Regex::new(ACTIONS_BLOCK_PATTERN).expect("static regex");
    let caps = re
        .captures(output)
        .ok_or_else(|| anyhow!("missing <!-- actions --> block"))?;
    let json_str = caps
        .get(1)
        .map(|m| m.as_str())
        .ok_or_else(|| anyhow!("invalid <!-- actions --> block"))?;
    let actions: Vec<OrchestratorAction> =
        serde_json::from_str(json_str).context("failed to parse actions JSON")?;
    if actions.is_empty() {
        bail!("actions block was empty");
    }
    Ok(actions)
}

pub fn enforce_guardrails(
    actions: &[OrchestratorAction],
    orchestrator: &OrchestratorConfig,
) -> Result<()> {
    if actions.len() > orchestrator.max_actions_per_turn {
        bail!(
            "action count {} exceeds max_actions_per_turn {}",
            actions.len(),
            orchestrator.max_actions_per_turn
        );
    }

    let subtask_count = actions
        .iter()
        .filter(|action| matches!(action, OrchestratorAction::Subtask { .. }))
        .count();
    if subtask_count > orchestrator.max_subtasks {
        bail!(
            "subtask action count {} exceeds max_subtasks {}",
            subtask_count,
            orchestrator.max_subtasks
        );
    }

    for action in actions {
        if let OrchestratorAction::Run { tool, .. } = action {
            if !orchestrator
                .allowed_tools
                .iter()
                .any(|allowed| is_allowed_tool_match(tool, allowed))
            {
                bail!("run action requested disallowed tool `{tool}`");
            }
        }
    }

    Ok(())
}

fn is_allowed_tool_match(tool: &str, allowed: &str) -> bool {
    if allowed == tool {
        return true;
    }
    if !allowed.contains('/') {
        return tool
            .strip_prefix(allowed)
            .is_some_and(|suffix| suffix.starts_with('/'));
    }
    if let Some(prefix) = allowed.strip_suffix("/*") {
        return tool
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'));
    }
    false
}

pub fn parse_duration_spec(raw: &str) -> Result<Duration> {
    let trimmed = raw.trim();
    let re = Regex::new(r"^(\d+)([smhd])$").expect("static regex");
    let caps = re
        .captures(trimmed)
        .ok_or_else(|| anyhow!("invalid duration `{trimmed}`"))?;
    let amount = caps
        .get(1)
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .ok_or_else(|| anyhow!("invalid duration value `{trimmed}`"))?;
    let unit = caps
        .get(2)
        .map(|m| m.as_str())
        .ok_or_else(|| anyhow!("invalid duration unit `{trimmed}`"))?;

    if amount <= 0 {
        bail!("duration must be > 0");
    }

    let duration = match unit {
        "s" => Duration::seconds(amount),
        "m" => Duration::minutes(amount),
        "h" => Duration::hours(amount),
        "d" => Duration::days(amount),
        _ => bail!("unsupported duration unit `{unit}`"),
    };
    Ok(duration)
}

pub fn resolve_deadline(
    deadline: &str,
    created_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(deadline) {
        return Ok(parsed.with_timezone(&Utc));
    }

    let duration = parse_duration_spec(deadline)?;
    Ok(created_at.unwrap_or(now) + duration)
}

pub async fn execute_actions(
    config: &Config,
    db: &Database,
    task: &AgentTask,
    orchestrator: &OrchestratorConfig,
    actions: &[OrchestratorAction],
) -> Result<ActionExecutionOutcome> {
    enforce_guardrails(actions, orchestrator)?;

    let mut outcome = ActionExecutionOutcome::default();
    let mut next_turn_notes: Vec<String> = Vec::new();

    for action in actions {
        match action {
            OrchestratorAction::Reply { text } => {
                db.insert_log(
                    task.id,
                    "orchestrator",
                    &format!("Action reply: {}", text.trim()),
                    "info",
                    None,
                )
                .await?;
                let _ = crate::slack::send_task_message(config, db, task, text).await?;
            }
            OrchestratorAction::Run { tool, args } => {
                db.insert_log(
                    task.id,
                    "orchestrator",
                    &format!("Action run accepted for tool `{tool}`"),
                    "info",
                    None,
                )
                .await?;

                // v1: the executor enforces allowlists/caps, then feeds the approved
                // run request into the next turn so the orchestrator can call MCP
                // tools inline in-session with full context.
                let mut note = format!("Approved run action: call `{tool}` next turn");
                if !args.is_null() {
                    let _ = write!(&mut note, " with args `{}`", args);
                }
                note.push_str(
                    ". After calling it, summarize the result and emit the next <!-- actions --> block.",
                );
                next_turn_notes.push(note);
            }
            OrchestratorAction::Defer { in_for, note } => {
                let duration = parse_duration_spec(in_for)?;
                let until = Utc::now() + duration;
                db.set_not_before(task.id, Some(until)).await?;
                db.insert_log(
                    task.id,
                    "orchestrator",
                    &format!("Action defer: waiting until {}", until.to_rfc3339()),
                    "info",
                    None,
                )
                .await?;
                next_turn_notes.push(
                    note.clone().unwrap_or_else(|| {
                        format!("Deferred for {in_for}. Resume from this state.")
                    }),
                );
                outcome.deferred_until = Some(until);
            }
            OrchestratorAction::Subtask {
                title,
                description,
                kind,
                repo,
            } => {
                let subtask_kind = kind.as_deref().unwrap_or(task.kind.as_str());
                let parsed_kind = subtask_kind.parse::<TaskKind>().unwrap_or(TaskKind::Ops);
                let subtask_repo = repo.as_deref().or(task.repo.as_deref());
                if parsed_kind == TaskKind::Code && subtask_repo.is_none() {
                    bail!("code subtasks require a repo");
                }

                let subtask = db
                    .create_subtask(
                        task.id,
                        task.user_id,
                        title,
                        description.as_deref(),
                        subtask_repo,
                        &task.priority,
                        task.cli.as_deref(),
                        task.model.as_deref(),
                        Some(parsed_kind.as_str()),
                    )
                    .await?;
                db.insert_log(
                    task.id,
                    "orchestrator",
                    &format!("Action subtask: created {}", subtask.id),
                    "info",
                    None,
                )
                .await?;
            }
            OrchestratorAction::Escalate { text, channel } => {
                db.insert_log(
                    task.id,
                    "orchestrator",
                    &format!("Action escalate: {}", text.trim()),
                    "warning",
                    None,
                )
                .await?;
                if let Some(channel) = channel {
                    let _ = crate::slack::send_message(config, channel, None, text).await;
                } else {
                    let _ = crate::slack::send_task_message(config, db, task, text).await?;
                }
            }
            OrchestratorAction::Close { reason } => {
                let close_reason = reason
                    .clone()
                    .unwrap_or_else(|| "closed by orchestrator action".to_string());
                db.insert_log(
                    task.id,
                    "orchestrator",
                    &format!("Action close: {close_reason}"),
                    "info",
                    None,
                )
                .await?;
                db.set_not_before(task.id, None).await?;
                db.update_status(task.id, TaskStatus::Done.as_str()).await?;
                if !close_reason.trim().is_empty() {
                    db.set_feedback(task.id, &close_reason).await?;
                    outcome.wrote_feedback = true;
                }
                outcome.closed = true;
                break;
            }
        }
    }

    if !outcome.closed && !next_turn_notes.is_empty() {
        let note = next_turn_notes.join("\n\n");
        db.set_feedback(task.id, &note).await?;
        outcome.wrote_feedback = true;
    }

    Ok(outcome)
}

pub async fn escalate_and_close(
    config: &Config,
    db: &Database,
    task: &AgentTask,
    reason: &str,
) -> Result<()> {
    let text = format!("Orchestrator limit reached: {reason}");
    db.insert_log(task.id, "orchestrator", &text, "warning", None)
        .await?;
    if let Err(error) =
        crate::slack::send_task_message(config, db, task, &format!(":warning: {text}")).await
    {
        warn!(task_id = %task.id, %error, "failed to send orchestrator escalation to Slack");
    }
    db.set_feedback(task.id, reason).await?;
    db.set_not_before(task.id, None).await?;
    db.update_status(task.id, TaskStatus::Done.as_str()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        enforce_guardrails, is_allowed_tool_match, parse_actions_from_output, parse_duration_spec,
        OrchestratorAction,
    };
    use crate::models::OrchestratorConfig;

    #[test]
    fn parses_actions_block() {
        let output = r#"Intro
<!-- actions
[
  {"type":"reply","text":"Checking now"},
  {"type":"defer","in":"15m","note":"Recheck later"}
]
actions -->
"#;
        let actions = parse_actions_from_output(output).expect("actions should parse");
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], OrchestratorAction::Reply { .. }));
        assert!(matches!(actions[1], OrchestratorAction::Defer { .. }));
    }

    #[test]
    fn parses_duration_units() {
        assert_eq!(parse_duration_spec("30s").unwrap().num_seconds(), 30);
        assert_eq!(parse_duration_spec("15m").unwrap().num_minutes(), 15);
        assert_eq!(parse_duration_spec("2h").unwrap().num_hours(), 2);
    }

    #[test]
    fn rejects_disallowed_run_tool() {
        let actions = vec![OrchestratorAction::Run {
            tool: "node-watchman/fal_run_test".to_string(),
            args: serde_json::json!({"node":"N1"}),
        }];
        let cfg = OrchestratorConfig {
            allowed_tools: vec!["other/tool".to_string()],
            ..OrchestratorConfig::default()
        };
        let err = enforce_guardrails(&actions, &cfg).expect_err("tool should be rejected");
        assert!(err
            .to_string()
            .contains("run action requested disallowed tool"));
    }

    #[test]
    fn rejects_too_many_actions() {
        let actions = vec![
            OrchestratorAction::Reply {
                text: "one".to_string(),
            },
            OrchestratorAction::Reply {
                text: "two".to_string(),
            },
        ];
        let cfg = OrchestratorConfig {
            max_actions_per_turn: 1,
            ..OrchestratorConfig::default()
        };
        let err = enforce_guardrails(&actions, &cfg).expect_err("action cap should be enforced");
        assert!(err.to_string().contains("max_actions_per_turn"));
    }

    #[test]
    fn matches_server_wide_allowed_tools() {
        assert!(is_allowed_tool_match(
            "node-watchman/fal_run_test",
            "node-watchman"
        ));
        assert!(is_allowed_tool_match(
            "node-watchman/fal_run_test",
            "node-watchman/*"
        ));
        assert!(!is_allowed_tool_match(
            "node-watchman/fal_run_test",
            "victoriametrics"
        ));
    }
}
