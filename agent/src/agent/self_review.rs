//! Self-review stage: a reviewer agent inspects the working-tree diff before
//! Karna commits and opens the PR.
//!
//! The reviewer runs read-only in the worktree and emits a delimited-section
//! contract (`===FLAGS===` / `===VERDICT===` / `===CHANGES===`). The verdict is
//! parsed tool-agnostically so any backend (claude/codex/cursor/grok/opencode)
//! can drive the stage. On `CHANGES`, the implementer is re-invoked with the
//! review notes; the loop is bounded by `config.max_review_rounds`.

use anyhow::Result;
use tracing::info;

use crate::cli::{self, CliOptions};
use crate::config::Config;
use crate::db::Database;
use crate::models::AgentTask;

/// Outcome of one review round.
#[derive(Debug, Clone)]
pub enum Verdict {
    /// Diff is correct and complete — ship it.
    Approve,
    /// Changes required; carries the reviewer's actionable notes.
    Changes(String),
}

/// Maximum diff characters to embed in the review prompt. Large diffs are
/// truncated to stay within context limits; the reviewer can still open files.
const MAX_DIFF_CHARS: usize = 60_000;

/// Run a single review round over `diff` for `task`, returning the verdict.
///
/// `working_dir` is the worktree the reviewer reads from. The reviewer stage's
/// tool/model is resolved via `Stage::Review` (per-stage profile → assigned →
/// default). Failures to parse a verdict default to `Approve` so a confused
/// reviewer never blocks shipping or spins the loop forever.
pub async fn review_diff(
    config: &Config,
    db: &Database,
    task: &AgentTask,
    working_dir: &std::path::Path,
    diff: &str,
    round: u32,
) -> Result<Verdict> {
    let (cli_name, model, addendum) =
        super::resolve_runtime(db, task, config, super::Stage::Review).await;
    let system_prompt =
        super::merge_system_prompt(config.instructions.as_deref(), addendum.as_deref());

    let diff_for_prompt = if diff.len() > MAX_DIFF_CHARS {
        format!(
            "{}\n\n[... diff truncated at {} chars; use Read/Grep to inspect the rest ...]",
            &diff[..MAX_DIFF_CHARS],
            MAX_DIFF_CHARS
        )
    } else {
        diff.to_string()
    };

    let description = task
        .description
        .as_deref()
        .unwrap_or("No description provided.");
    let plan = task.plan_content.as_deref().unwrap_or("No plan available.");

    let mut prompt = include_str!("../../templates/review_prompt.txt").to_string();
    prompt = prompt.replace("{title}", &task.title);
    prompt = prompt.replace("{description}", description);
    prompt = prompt.replace("{plan}", plan);
    prompt = prompt.replace("{diff}", &diff_for_prompt);

    db.insert_log(
        task.id,
        "self_review",
        &format!("Invoking {cli_name} ({model}) for self-review (round {round})"),
        "command",
        None,
    )
    .await?;

    let event_tx = super::spawn_log_consumer(db.clone(), task.id, "self_review");

    let result = cli::run(
        &cli_name,
        CliOptions {
            working_dir,
            prompt: &prompt,
            system_prompt: system_prompt.as_deref(),
            allowed_tools: Some("Read,Glob,Grep,Bash"),
            max_turns: 30,
            model: &model,
            mcp_config_json: None,
            session_id: None,
            resume: false,
            event_tx: Some(event_tx),
            image_paths: Vec::new(),
        },
    )
    .await?;

    db.add_cost(task.id, result.cost_usd).await?;

    Ok(parse_verdict(&result.output))
}

/// Parse the `===VERDICT===` / `===CHANGES===` / `===FLAGS===` contract.
/// Unparseable or missing verdict → `Approve` (never block shipping).
fn parse_verdict(output: &str) -> Verdict {
    let verdict = section(output, "===VERDICT===")
        .unwrap_or_default()
        .to_ascii_uppercase();

    // Only treat as CHANGES when the verdict explicitly says so. A reviewer that
    // writes APPROVE (or omits the section) ships.
    let wants_changes = verdict.contains("CHANGES") && !verdict.contains("APPROVE");
    if !wants_changes {
        return Verdict::Approve;
    }

    let changes = section(output, "===CHANGES===").unwrap_or_default();
    let notes = if changes.trim().is_empty() {
        // Fall back to FLAGS if the model put its findings there instead.
        section(output, "===FLAGS===").unwrap_or_default()
    } else {
        changes
    };
    let notes = notes.trim();
    let no_actionable_notes =
        notes.is_empty() || notes.eq_ignore_ascii_case("none") || notes.eq_ignore_ascii_case("n/a");
    if no_actionable_notes {
        // CHANGES requested but no actionable notes — nothing to act on, ship.
        info!("Self-review said CHANGES but gave no actionable notes; treating as APPROVE");
        return Verdict::Approve;
    }
    Verdict::Changes(notes.to_string())
}

/// Extract the text of a `===MARKER===` section: everything after the marker up
/// to the next `===…===` marker (or end of output).
fn section(output: &str, marker: &str) -> Option<String> {
    let start = output.find(marker)? + marker.len();
    let rest = &output[start..];
    // Find the next delimiter line starting with "===".
    let end = rest
        .match_indices("===")
        .map(|(i, _)| i)
        .find(|&i| i > 0)
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approve_verdict() {
        let out = "===FLAGS===\nnone\n===VERDICT===\nAPPROVE\n===CHANGES===\n";
        assert!(matches!(parse_verdict(out), Verdict::Approve));
    }

    #[test]
    fn changes_verdict_with_notes() {
        let out = "===FLAGS===\n- bug in foo\n===VERDICT===\nCHANGES\n===CHANGES===\n1. Fix the null check in foo.rs";
        match parse_verdict(out) {
            Verdict::Changes(n) => assert!(n.contains("null check")),
            _ => panic!("expected Changes"),
        }
    }

    #[test]
    fn changes_without_notes_falls_back_to_flags() {
        let out = "===FLAGS===\n- missing error handling in bar()\n===VERDICT===\nCHANGES\n===CHANGES===\n";
        match parse_verdict(out) {
            Verdict::Changes(n) => assert!(n.contains("error handling")),
            _ => panic!("expected Changes from FLAGS fallback"),
        }
    }

    #[test]
    fn unparseable_defaults_to_approve() {
        let out = "I think this looks fine overall.";
        assert!(matches!(parse_verdict(out), Verdict::Approve));
    }

    #[test]
    fn changes_with_empty_notes_ships() {
        let out = "===FLAGS===\nnone\n===VERDICT===\nCHANGES\n===CHANGES===\n   ";
        assert!(matches!(parse_verdict(out), Verdict::Approve));
    }
}
