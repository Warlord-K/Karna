//! Auto-review human-opened PRs.
//!
//! Triggered by the GitHub `pull_request` webhook (action `opened`,
//! `synchronize`, `reopened`) for branches that do NOT start with `kar-`
//! (agent PRs are handled separately). Per-repo opt-in via
//! `repo_profiles.review_prs`. Uses the user's existing Claude/Codex
//! subscription — no extra API spend.
//!
//! The reviewer runs the CLI in read-only mode (no Edit/Write tools), with a
//! strict review-mode system prompt that limits comments to substantive
//! correctness / security / bug issues. It posts the review via
//! `gh pr review <pr> --comment --body "..."` directly from the CLI's Bash
//! tool — no parsing of structured output needed.

use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::cli::{self, CliOptions};
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
    // 1. Check per-repo opt-in
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

    // 2. Resolve which agent profile to use (repo override → first default profile → config default)
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

    // 3. Atomic dedupe via UNIQUE (repo, head_sha). If two webhook firings
    //    race, only one wins and the other gets None.
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

    let outcome = run_review(&config, &cli_name, &model, addendum.as_deref(), &req).await;

    let (status, cost_usd, error) = match &outcome {
        Ok(cost) => ("completed", *cost, None),
        Err(e) => ("failed", 0.0, Some(format!("{e:#}"))),
    };

    if let Err(e) = db
        .complete_pr_review(review.id, status, 0, cost_usd, error.as_deref())
        .await
    {
        warn!(error = %e, "Failed to record pr_review completion");
    }

    outcome.map(|_| ())
}

/// Runs the CLI review and returns the cost spent. The caller records the
/// outcome on the `pr_reviews` row.
async fn run_review(
    config: &Config,
    cli_name: &str,
    model: &str,
    addendum: Option<&str>,
    req: &ReviewRequest<'_>,
) -> Result<f64> {
    // Ensure the repo is cloned and fetched. Working dir is the base-branch
    // checkout — agent uses `gh pr diff` / `gh pr view` for the PR contents.
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

    let result = cli::run(
        cli_name,
        CliOptions {
            working_dir: &clone_path,
            prompt: &prompt,
            system_prompt: Some(&system_prompt),
            // Read-only review — no Write/Edit. Bash needed for `gh`.
            allowed_tools: Some("Read,Glob,Grep,Bash"),
            max_turns: 30,
            model,
            mcp_config_json: None,
            session_id: None,
            resume: false,
            event_tx: None,
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
