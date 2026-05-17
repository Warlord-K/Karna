//! Auto-review human-opened PRs.
//!
//! Triggered by the GitHub `pull_request` webhook (action `opened`,
//! `synchronize`, `reopened`) for branches that do NOT start with `kar-`
//! (agent PRs are handled separately). Per-repo opt-in via
//! `repo_profiles.review_prs`. Uses the user's existing Claude/Codex
//! subscription — no extra API spend.
//!
//! Lifecycle:
//!   1. Dedupe via UNIQUE (repo, head_sha) on `pr_reviews`.
//!   2. Post a "review in progress" comment so the author sees something is
//!      happening immediately (and rules out "did the webhook even fire?").
//!   3. Stream CLI tool calls + assistant text into `pr_review_logs` so the
//!      UI can show live progress.
//!   4. CLI posts the final review via `gh pr review --comment --body "..."`.
//!   5. Edit (or delete) the progress comment to reflect outcome.

use anyhow::{Context, Result};
use serde_json::json;
use tokio::process::Command;
use tracing::{info, warn};
use uuid::Uuid;

use crate::cli::{self, CliOptions, EventSender, StreamEvent};
use crate::config::Config;
use crate::db::Database;
use crate::git::workspace;

const REVIEWER_SYSTEM_PROMPT: &str = "\
You are Karna, reviewing a pull request opened by a human teammate. You are not \
the author — you are the senior reviewer.\n\n\
RULES — read carefully, these are not suggestions:\n\
1. Comment ONLY on substantive issues: bugs, security vulnerabilities, data \
   loss risks, broken edge cases, incorrect logic, race conditions, missing \
   error handling at system boundaries, and clear violations of the repo's \
   stated conventions (CLAUDE.md / AGENTS.md).\n\
2. DO NOT comment on style, naming, formatting, or anything that a linter \
   or type-checker already catches. If unsure whether a finding is \
   substantive, stay silent.\n\
3. DO NOT suggest refactors or speculative improvements. Stick to issues \
   in the diff as it stands.\n\
4. DO NOT approve or request changes — your review is comment-only.\n\
5. If the diff is clean, post a single short comment saying so. Do not \
   manufacture findings.\n\n\
HOW TO REVIEW:\n\
- Start with `gh pr diff <PR>` and `gh pr view <PR> --json files,title,body` \
  to see the change.\n\
- Read the surrounding code with Read/Glob/Grep to understand intent.\n\
- Read CLAUDE.md / AGENTS.md if present for repo-specific conventions.\n\n\
HOW TO POST:\n\
- Submit exactly ONE review using `gh pr review <PR> --comment --body \"...\"`.\n\
- Format the body as markdown. If you have multiple findings, use a bullet \
  list with file:line references. If clean, say \"Reviewed — looks good \
  from a correctness standpoint.\" or similar.\n\
- DO NOT use `gh pr review --approve` or `--request-changes`.\n\
- DO NOT post anything other than the single review submission.\n\n\
The repo is already checked out at the working directory. The CLI you have \
access to is `gh` (authenticated). All other tools are read-only.";

#[allow(clippy::too_many_arguments)]
pub struct ReviewRequest<'a> {
    pub repo: &'a str,
    pub pr_number: i32,
    pub pr_url: Option<&'a str>,
    pub head_sha: &'a str,
    pub author: Option<&'a str>,
    pub branch: &'a str,
}

/// Entry point — call from a tokio task spawned off the webhook handler.
/// Returns Ok(()) on every outcome (success, skip, even failure) — we record
/// the result on the `pr_reviews` row, so the caller doesn't need to act on it.
pub async fn maybe_review_pr(config: Config, db: Database, req: ReviewRequest<'_>) -> Result<()> {
    let profile = db
        .get_repo_profile(req.repo)
        .await
        .context("Failed to look up repo profile")?;
    let Some(profile) = profile else {
        info!(repo = req.repo, "No repo profile, skipping PR review");
        return Ok(());
    };
    if !profile.review_prs {
        info!(repo = req.repo, "PR review disabled for repo, skipping");
        return Ok(());
    }

    let agent_profile = if let Some(id) = profile.review_agent_id {
        db.get_agent_profile(id).await.ok().flatten()
    } else {
        None
    };

    let (cli_name, model, addendum) = match agent_profile.as_ref() {
        Some(p) => (p.cli.clone(), p.model.clone(), p.system_prompt_addendum.clone()),
        None => {
            let cli = config.default_cli().to_string();
            let model = config.default_model(&cli).to_string();
            (cli, model, None)
        }
    };

    // Race-safe insert. If two webhook firings collide on the same head_sha,
    // only one wins; the other returns None and exits cleanly.
    let review_row = db
        .start_pr_review(
            req.repo,
            req.pr_number,
            req.pr_url,
            req.head_sha,
            req.author,
            agent_profile.as_ref().map(|p| p.id),
        )
        .await
        .context("Failed to create pr_review row")?;

    let Some(review) = review_row else {
        info!(
            repo = req.repo,
            pr = req.pr_number,
            head_sha = req.head_sha,
            "PR review already in progress or completed for this commit, skipping"
        );
        return Ok(());
    };

    // Post the "in progress" comment so the author sees instant feedback.
    // Best-effort — a failure here doesn't stop the review.
    let agent_label = agent_profile
        .as_ref()
        .map(|p| format!("{} {}", p.avatar_emoji, p.name))
        .unwrap_or_else(|| format!("{cli_name} ({model})"));
    let progress_body = format!(
        "🤖 **Karna review in progress**\n\n\
         {agent_label} is reviewing this PR. The final review will appear as a separate comment below.\n\n\
         <sub>This comment will be updated when the review finishes. Sourced from <code>{head_sha}</code>.</sub>",
        head_sha = &req.head_sha[..req.head_sha.len().min(8)],
    );
    let progress_comment_id = match post_pr_comment(req.repo, req.pr_number, &progress_body).await {
        Ok(id) => {
            let _ = db.insert_pr_review_log(
                review.id,
                "review",
                &format!("Posted progress comment #{id} on {}#{}", req.repo, req.pr_number),
                "info",
                None,
            ).await;
            Some(id)
        }
        Err(e) => {
            warn!(error = %e, "Failed to post review-in-progress comment");
            let _ = db.insert_pr_review_log(
                review.id,
                "review",
                &format!("Failed to post progress comment: {e}"),
                "warning",
                None,
            ).await;
            None
        }
    };

    let _ = db.insert_pr_review_log(
        review.id,
        "review",
        &format!("Invoking {cli_name} ({model}) for read-only review"),
        "command",
        None,
    ).await;

    let outcome = run_review(&config, &db, review.id, &cli_name, &model, addendum.as_deref(), &req).await;

    let (status, cost_usd, error) = match &outcome {
        Ok(cost) => ("completed", *cost, None),
        Err(e) => ("failed", 0.0, Some(format!("{e:#}"))),
    };

    // Update or remove the progress comment to reflect outcome.
    if let Some(comment_id) = progress_comment_id {
        let final_body = match &outcome {
            Ok(_) => format!(
                "🤖 **Karna review complete** — see the review below.\n\n\
                 <sub>Reviewed by {agent_label} on commit <code>{head_sha}</code>.</sub>",
                head_sha = &req.head_sha[..req.head_sha.len().min(8)],
            ),
            Err(e) => {
                let msg = format!("{e:#}");
                let truncated: String = msg.chars().take(500).collect();
                format!(
                    "🤖 **Karna review failed**\n\n\
                     ```\n{truncated}\n```\n\n\
                     <sub>Reviewer: {agent_label}. Commit <code>{head_sha}</code>.</sub>",
                    head_sha = &req.head_sha[..req.head_sha.len().min(8)],
                )
            }
        };
        if let Err(e) = update_pr_comment(req.repo, comment_id, &final_body).await {
            warn!(error = %e, comment_id, "Failed to update progress comment");
        }
    }

    if let Err(e) = db
        .complete_pr_review(review.id, status, 0, cost_usd, error.as_deref())
        .await
    {
        warn!(error = %e, "Failed to record pr_review completion");
    }

    let _ = db.insert_pr_review_log(
        review.id,
        "review",
        &format!("Review finished with status={status}, cost_usd={cost_usd:.4}"),
        if status == "completed" { "info" } else { "error" },
        None,
    ).await;

    outcome.map(|_| ())
}

async fn run_review(
    config: &Config,
    db: &Database,
    review_id: Uuid,
    cli_name: &str,
    model: &str,
    addendum: Option<&str>,
    req: &ReviewRequest<'_>,
) -> Result<f64> {
    let clone_path = workspace::ensure_cloned(&config.repos_dir, req.repo, &config.github_token)
        .await
        .context("Failed to ensure repo is cloned for review")?;

    let system_prompt = match (config.instructions.as_deref(), addendum) {
        (Some(g), Some(a)) => format!("{REVIEWER_SYSTEM_PROMPT}\n\n{g}\n\n{a}"),
        (Some(g), None) => format!("{REVIEWER_SYSTEM_PROMPT}\n\n{g}"),
        (None, Some(a)) => format!("{REVIEWER_SYSTEM_PROMPT}\n\n{a}"),
        (None, None) => REVIEWER_SYSTEM_PROMPT.to_string(),
    };

    let author_line = req
        .author
        .map(|a| format!("PR author: @{a}\n"))
        .unwrap_or_default();
    let prompt = format!(
        "Review pull request #{pr} on {repo}.\n\n\
         {author_line}\
         Branch: {branch}\n\
         Commit: {sha}\n\n\
         Follow the review process described in your system instructions. \
         Use `gh pr diff {pr}` and `gh pr view {pr} --json files,title,body` \
         to see the change. Read surrounding code as needed. Post exactly one \
         review using `gh pr review {pr} --comment --body \"...\"`.",
        pr = req.pr_number,
        repo = req.repo,
        branch = req.branch,
        sha = req.head_sha,
    );

    info!(
        repo = req.repo,
        pr = req.pr_number,
        cli = cli_name,
        model,
        "Starting PR review"
    );

    let event_tx = spawn_review_log_consumer(db.clone(), review_id);

    let result = cli::run(
        cli_name,
        CliOptions {
            working_dir: &clone_path,
            prompt: &prompt,
            system_prompt: Some(&system_prompt),
            allowed_tools: Some("Read,Glob,Grep,Bash"),
            max_turns: 30,
            model,
            mcp_config_json: None,
            session_id: None,
            resume: false,
            event_tx: Some(event_tx),
            image_paths: Vec::new(),
        },
    )
    .await
    .context("CLI invocation failed during PR review")?;

    info!(
        pr = req.pr_number,
        cost_usd = result.cost_usd,
        "PR review submitted"
    );

    Ok(result.cost_usd)
}

/// Stream CLI events into the per-review log table. Mirrors
/// `agent::spawn_log_consumer` but persists to `pr_review_logs`.
fn spawn_review_log_consumer(db: Database, review_id: Uuid) -> EventSender {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let (message, log_type) = match event {
                StreamEvent::ToolUse { tool, input_summary } => {
                    let msg = if input_summary.is_empty() {
                        tool
                    } else {
                        format!("{tool}: {input_summary}")
                    };
                    (msg, "tool")
                }
                StreamEvent::AssistantText(text) => {
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
                .insert_pr_review_log(review_id, "review", &message, log_type, None)
                .await;
        }
    });
    tx
}

/// Post a comment on a PR via `gh api`. Returns the new comment's numeric ID.
async fn post_pr_comment(repo: &str, pr_number: i32, body: &str) -> Result<i64> {
    let output = Command::new("gh")
        .args([
            "api",
            &format!("repos/{repo}/issues/{pr_number}/comments"),
            "--method",
            "POST",
            "--input",
            "-",
            "--jq",
            ".id",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn gh api")?;

    // Write the JSON body on stdin to avoid shell-quoting issues with long messages.
    let payload = json!({ "body": body }).to_string();
    use tokio::io::AsyncWriteExt;
    {
        let mut child = output;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(payload.as_bytes()).await.ok();
            stdin.shutdown().await.ok();
        }
        let done = child.wait_with_output().await.context("gh api wait failed")?;
        if !done.status.success() {
            let stderr = String::from_utf8_lossy(&done.stderr);
            anyhow::bail!("gh api comment POST failed: {stderr}");
        }
        let id_str = String::from_utf8_lossy(&done.stdout);
        let id: i64 = id_str
            .trim()
            .parse()
            .with_context(|| format!("Comment ID parse failed: {id_str:?}"))?;
        Ok(id)
    }
}

/// Edit an existing PR comment by ID via `gh api`.
async fn update_pr_comment(repo: &str, comment_id: i64, body: &str) -> Result<()> {
    let mut child = Command::new("gh")
        .args([
            "api",
            &format!("repos/{repo}/issues/comments/{comment_id}"),
            "--method",
            "PATCH",
            "--input",
            "-",
            "--silent",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn gh api")?;

    let payload = json!({ "body": body }).to_string();
    use tokio::io::AsyncWriteExt;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(payload.as_bytes()).await.ok();
        stdin.shutdown().await.ok();
    }
    let done = child.wait_with_output().await.context("gh api wait failed")?;
    if !done.status.success() {
        let stderr = String::from_utf8_lossy(&done.stderr);
        anyhow::bail!("gh api comment PATCH failed: {stderr}");
    }
    Ok(())
}
