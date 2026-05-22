//! Auto-review human-opened PRs with inline-anchored comments.
//!
//! Triggered by the GitHub `pull_request` webhook (action `opened`,
//! `synchronize`, `reopened`) for branches that do NOT start with `kar-`
//! (agent PRs are handled separately). Per-repo opt-in via
//! `repo_profiles.review_prs`. Uses the user's existing Claude/Codex
//! subscription — no extra API spend.
//!
//! Posting model: the reviewer CLI is *no longer* allowed to call
//! `gh pr review` itself. Instead its final assistant message must contain a
//! `<!-- findings ... findings -->` JSON block. The agent parses that, validates
//! every anchor against the PR diff (so hallucinated line numbers get dropped
//! rather than rejected by GitHub mid-submission), and posts a single review
//! via `gh api repos/{owner}/{repo}/pulls/{n}/reviews` with the body + a
//! `comments[]` array. Every finding (posted or skipped) is persisted to
//! `pr_review_findings` so the UI can surface what got dropped.
//!
//! Lifecycle:
//!   1. Dedupe via UNIQUE (repo, head_sha) on `pr_reviews`.
//!   2. Post a "review in progress" comment so the author sees something is
//!      happening immediately.
//!   3. Stream CLI tool calls + assistant text into `pr_review_logs` so the
//!      UI can show live progress.
//!   4. Parse findings block from the CLI's final output; validate against the
//!      PR diff; persist findings; POST one structured review.
//!   5. Edit (or delete) the progress comment to reflect outcome.

use anyhow::{Context, Result};
use karna_shared::models::PrReview;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
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
5. If the diff is clean, post a clean summary with zero comments. Do not \
   manufacture findings.\n\n\
HOW TO REVIEW:\n\
- Start with `gh pr diff <PR>` and `gh pr view <PR> --json files,title,body` \
  to see the change.\n\
- Read the surrounding code with Read/Glob/Grep to understand intent.\n\
- Read CLAUDE.md / AGENTS.md if present for repo-specific conventions.\n\n\
HOW TO POST (CRITICAL — read every word):\n\
- DO NOT run `gh pr review`, `gh pr comment`, or any `gh api .../reviews` \
  command yourself. Posting is handled for you by the harness.\n\
- The LAST thing in your final assistant message MUST be a fenced findings \
  block in EXACTLY this format:\n\n\
  <!-- findings\n\
  {\n\
    \"summary\": \"<short markdown summary of the review — appears as the \
review body. If clean, say so here in one sentence.>\",\n\
    \"comments\": [\n\
      {\n\
        \"path\": \"src/foo.py\",\n\
        \"line\": 42,\n\
        \"side\": \"RIGHT\",\n\
        \"severity\": \"high\",\n\
        \"body\": \"<markdown explaining the issue at this specific line>\"\n\
      },\n\
      {\n\
        \"path\": \"src/bar.rs\",\n\
        \"start_line\": 100,\n\
        \"line\": 105,\n\
        \"side\": \"RIGHT\",\n\
        \"severity\": \"medium\",\n\
        \"body\": \"<markdown for a multi-line comment spanning lines 100-105>\"\n\
      }\n\
    ]\n\
  }\n\
  findings -->\n\n\
SEVERITY (REQUIRED — pick one per finding):\n\
- \"high\": correctness bugs the common path hits (data loss, security \
  vulnerabilities, broken auth/access checks, panics on normal input, \
  obvious regressions). Reviewer should block the merge on these.\n\
- \"medium\": real bugs on edge cases or under specific conditions \
  (race conditions, missing error handling at system boundaries, off-by-one \
  on uncommon inputs, partial-failure modes). Worth fixing before merge \
  but not a hard blocker.\n\
- \"low\": minor concerns the author may want to revisit — defensive \
  improvements, subtle invariants worth documenting, things that aren't \
  wrong today but could bite later. Author's call to fix or defer.\n\
If a finding doesn't clearly clear the bar for at least \"low\", drop it \
entirely. Don't pad the review with noise.\n\n\
ANCHOR RULES:\n\
- `line` is the line number in the NEW version of the file (post-change).\n\
- Use `side: \"RIGHT\"` for comments on additions or surrounding context \
  (this is the common case). Use `side: \"LEFT\"` only when commenting on a \
  removed line; in that case `line` is the line number in the OLD version.\n\
- For a multi-line comment, set `start_line` to the first line and `line` to \
  the last line of the range; both must be on the same side.\n\
- The line you anchor to MUST appear in the PR diff (additions, deletions, or \
  context lines shown by `gh pr diff`). Anchors outside the diff will be \
  silently dropped — do not waste a comment on them.\n\
- If you need to make a point that doesn't anchor cleanly to a line, put it \
  in the `summary` field instead. The summary is for high-level commentary; \
  `comments[]` is for line-specific findings.\n\n\
The repo is already checked out at the working directory. The CLI you have \
access to is `gh` (authenticated) for read-only inspection. All other tools \
are read-only.";

#[allow(clippy::too_many_arguments)]
pub struct ReviewRequest<'a> {
    pub repo: &'a str,
    pub pr_number: i32,
    pub pr_url: Option<&'a str>,
    pub head_sha: &'a str,
    pub author: Option<&'a str>,
}

/// Webhook entry point — does the cheap checks (repo profile, review_prs flag,
/// dedupe) and inserts a `pending` row that the agent's poll loop will pick up.
///
/// Called from both the api webhook handler (`/webhooks/github` on karna-api)
/// and the agent webhook handler (`/webhooks/github` on the agent's own port).
/// Both paths converge on the same `pr_reviews` queue, so cloud and local-dev
/// deployments behave identically.
pub async fn enqueue_review(db: &Database, req: ReviewRequest<'_>) -> Result<EnqueueOutcome> {
    let profile = db
        .get_repo_profile(req.repo)
        .await
        .context("Failed to look up repo profile")?;
    let Some(profile) = profile else {
        info!(repo = req.repo, "No repo profile, skipping PR review");
        return Ok(EnqueueOutcome::SkippedNoProfile);
    };
    if !profile.review_prs {
        info!(repo = req.repo, "PR review disabled for repo, skipping");
        return Ok(EnqueueOutcome::SkippedDisabled);
    }

    let row = db
        .enqueue_pr_review(
            req.repo,
            req.pr_number,
            req.pr_url,
            req.head_sha,
            req.author,
            profile.review_agent_id,
        )
        .await
        .context("Failed to enqueue pr_review row")?;

    let Some(review) = row else {
        info!(
            repo = req.repo,
            pr = req.pr_number,
            head_sha = req.head_sha,
            "PR review already exists for this commit, skipping"
        );
        return Ok(EnqueueOutcome::Deduped);
    };
    info!(
        repo = req.repo,
        pr = req.pr_number,
        head_sha = req.head_sha,
        review_id = %review.id,
        "Enqueued PR review"
    );
    Ok(EnqueueOutcome::Enqueued)
}

#[derive(Debug)]
pub enum EnqueueOutcome {
    Enqueued,
    Deduped,
    SkippedDisabled,
    SkippedNoProfile,
}

/// Poll-loop tick — claim every `pending` PR review and process it.
/// Runs sequentially within one agent process; the DB `FOR UPDATE SKIP LOCKED`
/// in `claim_pending_pr_review` makes it safe for multiple replicas to compete.
pub async fn run_pending_reviews(config: &Config, db: &Database) -> Result<()> {
    loop {
        let Some(row) = db
            .claim_pending_pr_review()
            .await
            .context("Failed to claim pending PR review")?
        else {
            return Ok(());
        };
        info!(
            review_id = %row.id,
            repo = %row.repo,
            pr = row.pr_number,
            "Picked up pending PR review"
        );
        if let Err(e) = process_review_row(config, db, &row).await {
            warn!(error = %e, repo = %row.repo, pr = row.pr_number, "PR review failed");
        }
    }
}

struct ReviewOutcome {
    cost_usd: f64,
    comments_posted: i32,
}

async fn process_review_row(config: &Config, db: &Database, review: &PrReview) -> Result<()> {
    let agent_profile = if let Some(id) = review.reviewer_agent_id {
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
        head_sha = &review.head_sha[..review.head_sha.len().min(8)],
    );
    let progress_comment_id = match post_pr_comment(&review.repo, review.pr_number, &progress_body).await {
        Ok(id) => {
            let _ = db.insert_pr_review_log(
                review.id,
                "review",
                &format!("Posted progress comment #{id} on {}#{}", review.repo, review.pr_number),
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

    let outcome = run_review(config, db, review, &cli_name, &model, addendum.as_deref()).await;

    let (status, cost_usd, comments_posted, error) = match &outcome {
        Ok(o) => ("completed", o.cost_usd, o.comments_posted, None),
        Err(e) => ("failed", 0.0, 0, Some(format!("{e:#}"))),
    };

    if let Some(comment_id) = progress_comment_id {
        let final_body = match &outcome {
            Ok(o) => {
                let count_note = if o.comments_posted > 0 {
                    format!(" with {} inline comment(s)", o.comments_posted)
                } else {
                    String::new()
                };
                format!(
                    "🤖 **Karna review complete**{count_note} — see the review below.\n\n\
                     <sub>Reviewed by {agent_label} on commit <code>{head_sha}</code>.</sub>",
                    head_sha = &review.head_sha[..review.head_sha.len().min(8)],
                )
            }
            Err(e) => {
                let msg = format!("{e:#}");
                let truncated: String = msg.chars().take(500).collect();
                format!(
                    "🤖 **Karna review failed**\n\n\
                     ```\n{truncated}\n```\n\n\
                     <sub>Reviewer: {agent_label}. Commit <code>{head_sha}</code>.</sub>",
                    head_sha = &review.head_sha[..review.head_sha.len().min(8)],
                )
            }
        };
        if let Err(e) = update_pr_comment(&review.repo, comment_id, &final_body).await {
            warn!(error = %e, comment_id, "Failed to update progress comment");
        }
    }

    if let Err(e) = db
        .complete_pr_review(review.id, status, comments_posted, cost_usd, error.as_deref())
        .await
    {
        warn!(error = %e, "Failed to record pr_review completion");
    }

    let _ = db.insert_pr_review_log(
        review.id,
        "review",
        &format!(
            "Review finished with status={status}, comments_posted={comments_posted}, cost_usd={cost_usd:.4}"
        ),
        if status == "completed" { "info" } else { "error" },
        None,
    ).await;

    outcome.map(|_| ())
}

async fn run_review(
    config: &Config,
    db: &Database,
    review: &PrReview,
    cli_name: &str,
    model: &str,
    addendum: Option<&str>,
) -> Result<ReviewOutcome> {
    let clone_path = workspace::ensure_cloned(&config.repos_dir, &review.repo, &config.github_token)
        .await
        .context("Failed to ensure repo is cloned for review")?;

    let system_prompt = match (config.instructions.as_deref(), addendum) {
        (Some(g), Some(a)) => format!("{REVIEWER_SYSTEM_PROMPT}\n\n{g}\n\n{a}"),
        (Some(g), None) => format!("{REVIEWER_SYSTEM_PROMPT}\n\n{g}"),
        (None, Some(a)) => format!("{REVIEWER_SYSTEM_PROMPT}\n\n{a}"),
        (None, None) => REVIEWER_SYSTEM_PROMPT.to_string(),
    };

    let author_line = review
        .author
        .as_deref()
        .map(|a| format!("PR author: @{a}\n"))
        .unwrap_or_default();
    let prompt = format!(
        "Review pull request #{pr} on {repo}.\n\n\
         {author_line}\
         Commit: {sha}\n\n\
         Follow the review process described in your system instructions. \
         Use `gh pr diff {pr}` and `gh pr view {pr} --json files,title,body` \
         to see the change. Read surrounding code as needed.\n\n\
         End your final message with the `<!-- findings ... findings -->` JSON \
         block. Do NOT call `gh pr review` or `gh api .../reviews` yourself — \
         posting is handled by the harness.",
        pr = review.pr_number,
        repo = review.repo,
        sha = review.head_sha,
    );

    info!(
        repo = %review.repo,
        pr = review.pr_number,
        cli = cli_name,
        model,
        "Starting PR review"
    );

    let event_tx = spawn_review_log_consumer(db.clone(), review.id);

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
        pr = review.pr_number,
        cost_usd = result.cost_usd,
        "Reviewer CLI finished — parsing findings"
    );

    // Parse the structured findings block from the CLI's final output.
    let parsed = match parse_findings(&result.output) {
        Ok(p) => p,
        Err(e) => {
            // Fall back to posting the raw output as a body-only review so the
            // human reviewer still sees something useful.
            warn!(error = %e, "Could not parse findings block — falling back to body-only review");
            let _ = db.insert_pr_review_log(
                review.id,
                "review",
                &format!("Could not parse findings block: {e}. Posting raw output as body-only review."),
                "warning",
                None,
            ).await;
            ParsedFindings {
                summary: result.output.clone(),
                comments: Vec::new(),
            }
        }
    };

    // Fetch the diff and build a (path, side, line) anchor index so we can
    // drop hallucinated line numbers before GitHub rejects the whole review.
    let diff_index = match fetch_diff(&review.repo, review.pr_number).await {
        Ok(diff_text) => DiffIndex::parse(&diff_text),
        Err(e) => {
            warn!(error = %e, "Could not fetch PR diff for anchor validation");
            let _ = db.insert_pr_review_log(
                review.id,
                "review",
                &format!("Could not fetch PR diff: {e}. Posting body only."),
                "warning",
                None,
            ).await;
            DiffIndex::empty()
        }
    };

    let (valid_comments, skipped) = validate_findings(&parsed.comments, &diff_index);

    // Persist every finding — both posted-eligible and skipped — so the UI can
    // surface what got dropped and why.
    for fc in &valid_comments {
        let sev = normalize_severity(&fc.severity);
        if let Err(e) = db
            .insert_pr_review_finding(
                review.id,
                &fc.path,
                fc.line,
                fc.start_line,
                &fc.side,
                &fc.body,
                sev,
                true,
                None,
            )
            .await
        {
            warn!(error = %e, "Failed to persist posted finding");
        }
    }
    for (fc, reason) in &skipped {
        let sev = normalize_severity(&fc.severity);
        if let Err(e) = db
            .insert_pr_review_finding(
                review.id,
                &fc.path,
                fc.line,
                fc.start_line,
                &fc.side,
                &fc.body,
                sev,
                false,
                Some(reason),
            )
            .await
        {
            warn!(error = %e, "Failed to persist skipped finding");
        }
    }

    if !skipped.is_empty() {
        let _ = db.insert_pr_review_log(
            review.id,
            "review",
            &format!(
                "Skipped {} finding(s) whose anchors are not in the PR diff",
                skipped.len()
            ),
            "warning",
            None,
        ).await;
    }

    // Compose the review body. Append a footer noting any skipped findings so
    // the PR author can still see the underlying concern even if we couldn't
    // anchor it inline.
    let mut body = parsed.summary.trim().to_string();
    if body.is_empty() {
        body = if valid_comments.is_empty() {
            "Reviewed — no substantive issues found.".to_string()
        } else {
            "See inline comments below.".to_string()
        };
    }
    if !skipped.is_empty() {
        body.push_str("\n\n---\n_The reviewer flagged additional findings that couldn't be anchored to the diff:_\n\n");
        for (fc, reason) in &skipped {
            let sev = normalize_severity(&fc.severity);
            body.push_str(&format!(
                "- {} `{}:{}` ({}): {}\n",
                severity_marker(sev),
                fc.path,
                fc.line,
                reason,
                first_line(&fc.body),
            ));
        }
    }

    // Submit the review.
    let comments_posted = match post_structured_review(
        &review.repo,
        review.pr_number,
        &body,
        &valid_comments,
    )
    .await
    {
        Ok(n) => {
            let _ = db.insert_pr_review_log(
                review.id,
                "review",
                &format!("Posted review with {n} inline comment(s)"),
                "info",
                None,
            ).await;
            n
        }
        Err(e) => {
            // Last-resort fallback: post the body alone, no inline comments.
            warn!(error = %e, "Structured review POST failed — falling back to body-only");
            let _ = db.insert_pr_review_log(
                review.id,
                "review",
                &format!("Structured review POST failed: {e}. Falling back to body-only."),
                "warning",
                None,
            ).await;
            post_structured_review(&review.repo, review.pr_number, &body, &[])
                .await
                .context("Body-only fallback review also failed")?;
            0
        }
    };

    info!(
        pr = review.pr_number,
        cost_usd = result.cost_usd,
        comments_posted,
        "PR review submitted"
    );

    Ok(ReviewOutcome {
        cost_usd: result.cost_usd,
        comments_posted,
    })
}

fn first_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or("").trim();
    if line.len() > 140 {
        format!("{}…", &line[..140])
    } else {
        line.to_string()
    }
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

// ---------------------------------------------------------------------------
// Findings parser
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ParsedFindings {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    comments: Vec<FindingComment>,
}

#[derive(Debug, Clone, Deserialize)]
struct FindingComment {
    path: String,
    line: i32,
    #[serde(default)]
    start_line: Option<i32>,
    #[serde(default = "default_side")]
    side: String,
    #[serde(default = "default_severity")]
    severity: String,
    body: String,
}

fn default_side() -> String {
    "RIGHT".to_string()
}

/// Findings without an explicit severity fall through to medium so legacy
/// reviewers (or a forgetful model) still produce something the UI can render.
fn default_severity() -> String {
    "medium".to_string()
}

/// Canonicalize to "high" | "medium" | "low" so a model that emits
/// "HIGH", "Sev: High", or "critical" doesn't trip the CHECK constraint.
fn normalize_severity(raw: &str) -> &'static str {
    let lowered = raw.trim().to_ascii_lowercase();
    if lowered.contains("high") || lowered.contains("critical") || lowered.contains("sev1") {
        "high"
    } else if lowered.contains("low") || lowered.contains("minor") || lowered.contains("nit") {
        "low"
    } else {
        "medium"
    }
}

/// Prepended to the inline-comment body so the severity is visible on GitHub
/// itself, not just in the karna UI. Emoji + bold lets it scan at a glance
/// without depending on markdown rendering quirks.
fn severity_marker(severity: &str) -> &'static str {
    match severity {
        "high" => "🔴 **Sev: High**",
        "low" => "🔵 **Sev: Low**",
        _ => "🟡 **Sev: Medium**",
    }
}

fn parse_findings(output: &str) -> Result<ParsedFindings> {
    let re = regex_lite::Regex::new(r"(?s)<!--\s*findings\s*\n(.*?)\nfindings\s*-->")
        .expect("static regex");
    let caps = re
        .captures(output)
        .ok_or_else(|| anyhow::anyhow!("no <!-- findings ... findings --> block in CLI output"))?;
    let json_str = caps.get(1).unwrap().as_str();
    let parsed: ParsedFindings = serde_json::from_str(json_str)
        .with_context(|| format!("Findings block is not valid JSON: {json_str:.200}"))?;
    Ok(parsed)
}

// ---------------------------------------------------------------------------
// Diff anchor index
// ---------------------------------------------------------------------------

/// Set of (path, side, line) tuples GitHub will accept for inline comments.
/// "RIGHT" anchors are line numbers in the new file; "LEFT" anchors are line
/// numbers in the old file. A comment on a context line is valid on both sides.
struct DiffIndex {
    anchors: HashMap<(String, char), HashSet<i32>>,
}

impl DiffIndex {
    fn empty() -> Self {
        Self { anchors: HashMap::new() }
    }

    fn parse(diff: &str) -> Self {
        let mut anchors: HashMap<(String, char), HashSet<i32>> = HashMap::new();
        let mut new_path: Option<String> = None;
        let mut old_path: Option<String> = None;
        let mut new_line: i32 = 0;
        let mut old_line: i32 = 0;
        let mut in_hunk = false;

        let hunk_re = regex_lite::Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@")
            .expect("static regex");

        for raw in diff.lines() {
            if let Some(rest) = raw.strip_prefix("+++ b/") {
                new_path = Some(rest.split('\t').next().unwrap_or(rest).to_string());
                in_hunk = false;
                continue;
            }
            if raw.starts_with("+++ ") {
                // /dev/null or other — treat as no new path (binary, deletion)
                new_path = None;
                in_hunk = false;
                continue;
            }
            if let Some(rest) = raw.strip_prefix("--- a/") {
                old_path = Some(rest.split('\t').next().unwrap_or(rest).to_string());
                in_hunk = false;
                continue;
            }
            if raw.starts_with("--- ") {
                old_path = None;
                in_hunk = false;
                continue;
            }
            if let Some(caps) = hunk_re.captures(raw) {
                old_line = caps[1].parse().unwrap_or(0);
                new_line = caps[2].parse().unwrap_or(0);
                in_hunk = true;
                continue;
            }
            if !in_hunk {
                continue;
            }
            let first = raw.chars().next();
            match first {
                Some(' ') => {
                    if let Some(p) = &new_path {
                        anchors
                            .entry((p.clone(), 'R'))
                            .or_default()
                            .insert(new_line);
                    }
                    if let Some(p) = &old_path {
                        anchors
                            .entry((p.clone(), 'L'))
                            .or_default()
                            .insert(old_line);
                    }
                    new_line += 1;
                    old_line += 1;
                }
                Some('+') => {
                    if let Some(p) = &new_path {
                        anchors
                            .entry((p.clone(), 'R'))
                            .or_default()
                            .insert(new_line);
                    }
                    new_line += 1;
                }
                Some('-') => {
                    if let Some(p) = &old_path {
                        anchors
                            .entry((p.clone(), 'L'))
                            .or_default()
                            .insert(old_line);
                    }
                    old_line += 1;
                }
                Some('\\') => {
                    // "\ No newline at end of file" — no counter advance
                }
                _ => {
                    // Anything else (e.g. between hunks/files) breaks the hunk
                    in_hunk = false;
                }
            }
        }
        Self { anchors }
    }

    fn allows(&self, path: &str, line: i32, side: char) -> bool {
        self.anchors
            .get(&(path.to_string(), side))
            .is_some_and(|set| set.contains(&line))
    }
}

fn validate_findings(
    comments: &[FindingComment],
    diff: &DiffIndex,
) -> (Vec<FindingComment>, Vec<(FindingComment, String)>) {
    let mut valid = Vec::new();
    let mut skipped = Vec::new();
    for c in comments {
        if c.body.trim().is_empty() || c.path.trim().is_empty() {
            skipped.push((c.clone(), "empty body or path".into()));
            continue;
        }
        let side_char = if c.side.eq_ignore_ascii_case("LEFT") { 'L' } else { 'R' };
        if let Some(start) = c.start_line {
            if start > c.line {
                skipped.push((c.clone(), format!("start_line ({start}) > line ({})", c.line)));
                continue;
            }
            if !diff.allows(&c.path, start, side_char) {
                skipped.push((c.clone(), format!("start_line {start} not in diff")));
                continue;
            }
        }
        if !diff.allows(&c.path, c.line, side_char) {
            skipped.push((c.clone(), format!("line {} not in diff", c.line)));
            continue;
        }
        valid.push(c.clone());
    }
    (valid, skipped)
}

// ---------------------------------------------------------------------------
// gh wrappers
// ---------------------------------------------------------------------------

async fn fetch_diff(repo: &str, pr_number: i32) -> Result<String> {
    let output = Command::new("gh")
        .args(["pr", "diff", &pr_number.to_string(), "--repo", repo])
        .output()
        .await
        .context("Failed to spawn gh pr diff")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh pr diff failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Submit a single PR review with body + (optionally) inline comments via the
/// REST API. We use `gh api` so the existing GITHUB_TOKEN/auth flow is reused
/// without spawning a separate HTTP client.
async fn post_structured_review(
    repo: &str,
    pr_number: i32,
    body: &str,
    comments: &[FindingComment],
) -> Result<i32> {
    let mut payload = json!({
        "event": "COMMENT",
        "body": body,
    });
    if !comments.is_empty() {
        let arr: Vec<Value> = comments.iter().map(comment_payload).collect();
        payload["comments"] = Value::Array(arr);
    }
    let payload_str = payload.to_string();

    let child = Command::new("gh")
        .args([
            "api",
            &format!("repos/{repo}/pulls/{pr_number}/reviews"),
            "--method",
            "POST",
            "--input",
            "-",
            "--silent",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn gh api for review POST")?;

    use tokio::io::AsyncWriteExt;
    let mut child = child;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(payload_str.as_bytes()).await.ok();
        stdin.shutdown().await.ok();
    }
    let done = child
        .wait_with_output()
        .await
        .context("gh api review POST wait failed")?;
    if !done.status.success() {
        let stderr = String::from_utf8_lossy(&done.stderr);
        anyhow::bail!("gh api review POST failed: {stderr}");
    }
    Ok(comments.len() as i32)
}

fn comment_payload(c: &FindingComment) -> Value {
    let side = if c.side.eq_ignore_ascii_case("LEFT") { "LEFT" } else { "RIGHT" };
    let sev = normalize_severity(&c.severity);
    // Prepend the severity marker so the inline comment on GitHub itself
    // shows the tier — without it, reviewers would only see the badge in the
    // karna modal and miss it when reading the PR on github.com.
    let body = format!("{}\n\n{}", severity_marker(sev), c.body);
    let mut obj = json!({
        "path": c.path,
        "line": c.line,
        "side": side,
        "body": body,
    });
    if let Some(start) = c.start_line {
        if start != c.line {
            obj["start_line"] = json!(start);
            obj["start_side"] = json!(side);
        }
    }
    obj
}

/// Post a comment on a PR via `gh api`. Returns the new comment's numeric ID.
async fn post_pr_comment(repo: &str, pr_number: i32, body: &str) -> Result<i64> {
    let child = Command::new("gh")
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

    let payload = json!({ "body": body }).to_string();
    use tokio::io::AsyncWriteExt;
    let mut child = child;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_findings_block_basic() {
        let output = "Some preamble.\n\n<!-- findings\n{\n  \"summary\": \"All good\",\n  \"comments\": []\n}\nfindings -->\n";
        let p = parse_findings(output).unwrap();
        assert_eq!(p.summary, "All good");
        assert!(p.comments.is_empty());
    }

    #[test]
    fn parse_findings_with_inline_comments() {
        let output = "<!-- findings\n{\n  \"summary\": \"Two issues\",\n  \"comments\": [\n    {\"path\": \"src/foo.rs\", \"line\": 10, \"side\": \"RIGHT\", \"severity\": \"high\", \"body\": \"bug here\"},\n    {\"path\": \"src/bar.rs\", \"start_line\": 5, \"line\": 8, \"side\": \"RIGHT\", \"body\": \"multi line, no severity\"}\n  ]\n}\nfindings -->";
        let p = parse_findings(output).unwrap();
        assert_eq!(p.comments.len(), 2);
        assert_eq!(p.comments[0].path, "src/foo.rs");
        assert_eq!(p.comments[0].severity, "high");
        assert_eq!(p.comments[1].start_line, Some(5));
        // Missing severity falls through to medium via default_severity()
        assert_eq!(p.comments[1].severity, "medium");
    }

    #[test]
    fn normalize_severity_handles_variants() {
        assert_eq!(normalize_severity("high"), "high");
        assert_eq!(normalize_severity("HIGH"), "high");
        assert_eq!(normalize_severity("Sev: High"), "high");
        assert_eq!(normalize_severity("critical"), "high");
        assert_eq!(normalize_severity("low"), "low");
        assert_eq!(normalize_severity("nit"), "low");
        assert_eq!(normalize_severity("minor"), "low");
        assert_eq!(normalize_severity(""), "medium");
        assert_eq!(normalize_severity("medium"), "medium");
        // Unrecognized values fall through to medium so the CHECK constraint
        // never blocks a finding from being persisted.
        assert_eq!(normalize_severity("p2"), "medium");
    }

    #[test]
    fn comment_payload_prepends_severity_marker() {
        let c = FindingComment {
            path: "src/foo.rs".into(),
            line: 10,
            start_line: None,
            side: "RIGHT".into(),
            severity: "high".into(),
            body: "real issue".into(),
        };
        let p = comment_payload(&c);
        let body = p.get("body").unwrap().as_str().unwrap();
        assert!(body.starts_with("🔴 **Sev: High**"));
        assert!(body.contains("real issue"));
    }

    #[test]
    fn diff_index_tracks_added_and_context_lines() {
        let diff = concat!(
            "diff --git a/src/foo.rs b/src/foo.rs\n",
            "--- a/src/foo.rs\n",
            "+++ b/src/foo.rs\n",
            "@@ -10,3 +10,4 @@ context\n",
            " fn one() {}\n",
            "+fn two() {}\n",
            " fn three() {}\n",
            " fn four() {}\n",
        );
        let idx = DiffIndex::parse(diff);
        // new-side: context lines + added line are anchorable
        assert!(idx.allows("src/foo.rs", 10, 'R'));
        assert!(idx.allows("src/foo.rs", 11, 'R')); // the +fn two() line
        assert!(idx.allows("src/foo.rs", 12, 'R'));
        // Line outside the hunk is not anchorable
        assert!(!idx.allows("src/foo.rs", 99, 'R'));
        // Other paths are not anchorable
        assert!(!idx.allows("src/other.rs", 10, 'R'));
    }

    #[test]
    fn validate_drops_out_of_diff_anchors() {
        let diff = concat!(
            "diff --git a/src/foo.rs b/src/foo.rs\n",
            "--- a/src/foo.rs\n",
            "+++ b/src/foo.rs\n",
            "@@ -1,1 +1,2 @@\n",
            " fn one() {}\n",
            "+fn two() {}\n",
        );
        let idx = DiffIndex::parse(diff);
        let comments = vec![
            FindingComment {
                path: "src/foo.rs".into(),
                line: 2,
                start_line: None,
                side: "RIGHT".into(),
                severity: "high".into(),
                body: "real".into(),
            },
            FindingComment {
                path: "src/foo.rs".into(),
                line: 999,
                start_line: None,
                side: "RIGHT".into(),
                severity: "medium".into(),
                body: "hallucinated".into(),
            },
            FindingComment {
                path: "src/never_touched.rs".into(),
                line: 1,
                start_line: None,
                side: "RIGHT".into(),
                severity: "low".into(),
                body: "wrong file".into(),
            },
        ];
        let (valid, skipped) = validate_findings(&comments, &idx);
        assert_eq!(valid.len(), 1);
        assert_eq!(skipped.len(), 2);
    }
}
