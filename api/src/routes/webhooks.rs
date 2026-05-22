use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::{info, warn};

use karna_shared::models::TaskStatus;

use crate::AppState;

type HmacSha256 = Hmac<Sha256>;

pub async fn github_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> StatusCode {
    // Verify webhook signature if a secret is configured
    let webhook_secret = std::env::var("GITHUB_WEBHOOK_SECRET").ok();
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok());

    if !verify_signature(webhook_secret.as_deref(), signature, body.as_bytes()) {
        warn!("Webhook signature verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let payload: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let action = payload["action"].as_str().unwrap_or("");

    // Handle GitHub issue opened → create task
    let event = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if event == "issues" && action == "opened" {
        return handle_issue_opened(&state, &payload).await;
    }

    // Top-level bot filter — drop comments/reviews authored by bots
    // (Vercel preview deploys, GitHub Actions, dependabot, renovate, Karna's
    // own progress-comment edits). Mirrors the agent's webhook filter.
    if event == "issue_comment" || event == "pull_request_review_comment" {
        let kind = payload.pointer("/comment/user/type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "Bot" {
            let login = payload.pointer("/comment/user/login").and_then(|v| v.as_str()).unwrap_or("");
            info!(event, bot = login, "Webhook: ignoring bot comment");
            return StatusCode::OK;
        }
    }
    if event == "pull_request_review" {
        let kind = payload.pointer("/review/user/type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "Bot" {
            let login = payload.pointer("/review/user/login").and_then(|v| v.as_str()).unwrap_or("");
            info!(event, bot = login, "Webhook: ignoring bot review");
            return StatusCode::OK;
        }
    }

    let branch = payload
        .pointer("/pull_request/head/ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let is_agent_branch = branch.contains('/') && branch.split('/').next().is_some_and(|p| {
        p.rfind('-').is_some_and(|i| p[i + 1..].chars().all(|c| c.is_ascii_digit()) && i > 0)
    });
    if !is_agent_branch {
        // Human-opened PR → enqueue an auto-review if the repo opted in.
        if event == "pull_request" && matches!(action, "opened" | "reopened" | "synchronize") {
            return handle_pr_review_trigger(&state, &payload).await;
        }
        return StatusCode::OK;
    }

    let task = match state.db.find_task_by_branch(branch).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            warn!(branch, "Webhook: no task found for branch");
            return StatusCode::OK;
        }
        Err(e) => {
            warn!(error = %e, "Webhook: DB error");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // PR merged → mark task as done
    if action == "closed"
        && payload
            .pointer("/pull_request/merged")
            .and_then(|v| v.as_bool())
            == Some(true)
    {
        info!(task_id = %task.id, branch, "Webhook: PR merged, marking done");
        let _ = state.db.update_status(task.id, TaskStatus::Done.as_str()).await;
        let _ = state
            .db
            .insert_log(task.id, "webhook", "PR merged, task complete", "info", None)
            .await;
        return StatusCode::OK;
    }

    // PR review: changes requested → set feedback and move to in_progress
    if action == "submitted" {
        let review_state = payload
            .pointer("/review/state")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if review_state == "changes_requested" || review_state == "commented" {
            let review_body = payload
                .pointer("/review/body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if !review_body.is_empty() {
                info!(task_id = %task.id, branch, review_state, "Webhook: review feedback received");
                let _ = state.db.set_feedback(task.id, &review_body).await;
                let _ = state
                    .db
                    .update_status(task.id, TaskStatus::InProgress.as_str())
                    .await;
                let _ = state
                    .db
                    .insert_log(task.id, "webhook", &format!("PR review ({review_state}): feedback received"), "info", None)
                    .await;
            }
            return StatusCode::OK;
        }

        if review_state == "approved" {
            info!(task_id = %task.id, branch, "Webhook: PR approved");
            let _ = state
                .db
                .insert_log(task.id, "webhook", "PR approved", "info", None)
                .await;
            return StatusCode::OK;
        }
    }

    // Issue comment on a tracked PR → append to feedback
    if action == "created" && payload.get("comment").is_some() && payload.get("issue").is_some() {
        let comment_body = payload
            .pointer("/comment/body")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if !comment_body.is_empty() {
            info!(task_id = %task.id, "Webhook: PR comment received");
            let existing = task.feedback.as_deref().unwrap_or("");
            let combined = if existing.is_empty() {
                comment_body.to_string()
            } else {
                format!("{existing}\n\n---\n\n{comment_body}")
            };
            let _ = state.db.set_feedback(task.id, &combined).await;
            let _ = state
                .db
                .insert_log(task.id, "webhook", "PR comment added to feedback", "info", None)
                .await;
        }
    }

    StatusCode::OK
}

/// Enqueue an auto-review for a human-opened PR. The agent's poll loop claims
/// `pending` rows and runs the actual CLI-driven review (`agent/src/reviewer.rs`).
/// Decoupling the webhook from execution means cloud deploys (where the api
/// receives webhooks via ALB) and local docker-compose (where the agent's own
/// port is exposed) behave identically.
async fn handle_pr_review_trigger(
    state: &AppState,
    payload: &serde_json::Value,
) -> StatusCode {
    if payload
        .pointer("/pull_request/draft")
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        info!("Webhook: skipping PR review for draft");
        return StatusCode::OK;
    }

    let repo = match payload.pointer("/repository/full_name").and_then(|v| v.as_str()) {
        Some(r) => r.to_string(),
        None => return StatusCode::OK,
    };
    let pr_number = match payload.pointer("/pull_request/number").and_then(|v| v.as_i64()) {
        Some(n) => n as i32,
        None => return StatusCode::OK,
    };
    let pr_url = payload
        .pointer("/pull_request/html_url")
        .and_then(|v| v.as_str());
    let head_sha = match payload.pointer("/pull_request/head/sha").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return StatusCode::OK,
    };
    let author = payload
        .pointer("/pull_request/user/login")
        .and_then(|v| v.as_str());

    let profile = match state.db.get_repo_profile(&repo).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            info!(repo = %repo, pr = pr_number, "No profile for repo, skipping PR review");
            return StatusCode::OK;
        }
        Err(e) => {
            warn!(error = %e, "Failed to look up repo profile for PR review");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    if !profile.review_prs {
        info!(repo = %repo, pr = pr_number, "PR review disabled for repo, skipping");
        return StatusCode::OK;
    }

    match state
        .db
        .enqueue_pr_review(
            &repo,
            pr_number,
            pr_url,
            head_sha,
            author,
            profile.review_agent_id,
        )
        .await
    {
        Ok(Some(row)) => {
            info!(
                repo = %repo,
                pr = pr_number,
                head_sha,
                review_id = %row.id,
                "Enqueued PR review"
            );
            StatusCode::ACCEPTED
        }
        Ok(None) => {
            info!(repo = %repo, pr = pr_number, head_sha, "PR review already exists for commit");
            StatusCode::OK
        }
        Err(e) => {
            warn!(error = %e, "Failed to enqueue PR review");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn handle_issue_opened(state: &AppState, payload: &serde_json::Value) -> StatusCode {
    let repo_name = match payload.pointer("/repository/full_name").and_then(|v| v.as_str()) {
        Some(r) => r,
        None => return StatusCode::OK,
    };

    let sync_enabled = match state.db.get_repo_sync_issues(repo_name).await {
        Ok(enabled) => enabled,
        Err(_) => return StatusCode::OK,
    };
    if !sync_enabled {
        info!(repo = repo_name, "Issue sync disabled for repo, skipping");
        return StatusCode::OK;
    }

    let issue_number = payload.pointer("/issue/number").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let issue_title = payload.pointer("/issue/title").and_then(|v| v.as_str()).unwrap_or("Untitled");
    let issue_body = payload.pointer("/issue/body").and_then(|v| v.as_str()).unwrap_or("");
    let issue_url = payload.pointer("/issue/html_url").and_then(|v| v.as_str()).unwrap_or("");

    // Deduplicate: check if a task already exists for this issue
    match state.db.find_task_by_github_issue(repo_name, issue_number).await {
        Ok(Some(_)) => {
            info!(repo = repo_name, issue_number, "Task already exists for issue, skipping");
            return StatusCode::OK;
        }
        Err(e) => {
            warn!(error = %e, "Failed to check issue deduplication");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
        Ok(None) => {}
    }

    let user_id = match state.db.first_user_id().await {
        Ok(Some(id)) => id,
        _ => {
            warn!("No user found to assign issue task");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let title = format!("GH-{}: {}", issue_number, issue_title);
    let body_truncated = if issue_body.len() > 10_000 { &issue_body[..10_000] } else { issue_body };
    let description = if issue_url.is_empty() {
        body_truncated.to_string()
    } else {
        format!("{}\n\n---\n_Opened from: {}_", body_truncated, issue_url)
    };

    match state.db.create_task(user_id, &title, Some(&description), Some(repo_name), "medium", None, None).await {
        Ok(task) => {
            info!(task_id = %task.id, repo = repo_name, issue_number, "Created task from GitHub issue");
            let _ = state.db.insert_log(task.id, "webhook", &format!("Task created from GitHub issue #{issue_number}"), "info", None).await;
            StatusCode::OK
        }
        Err(e) => {
            warn!(error = %e, "Failed to create task from issue");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn verify_signature(secret: Option<&str>, signature_header: Option<&str>, body: &[u8]) -> bool {
    let secret = match secret {
        Some(s) => s,
        None => return true, // No secret configured — accept all
    };

    let signature = match signature_header {
        Some(s) => s,
        None => {
            warn!("Webhook missing X-Hub-Signature-256 header");
            return false;
        }
    };

    let expected = match signature.strip_prefix("sha256=") {
        Some(hex_sig) => hex_sig,
        None => return false,
    };

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let computed = hex::encode(mac.finalize().into_bytes());

    computed == expected
}

/// Compare a raw hex HMAC-SHA256 (no "sha256=" prefix) — Linear & ClickUp format.
fn verify_raw_hmac(secret: Option<&str>, signature_header: Option<&str>, body: &[u8]) -> bool {
    let secret = match secret {
        Some(s) => s,
        None => return true, // No secret configured — accept all
    };
    let signature = match signature_header {
        Some(s) => s,
        None => {
            warn!("Webhook missing signature header");
            return false;
        }
    };

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let computed = hex::encode(mac.finalize().into_bytes());

    // Linear sends the hex digest directly; tolerate "sha256=" prefix just in case.
    let expected = signature.strip_prefix("sha256=").unwrap_or(signature);
    computed == expected
}

// --- Linear ---
//
// Linear webhook payload (Issue create):
// {
//   "action": "create",
//   "type": "Issue",
//   "data": {
//     "id": "uuid",
//     "identifier": "ENG-123",
//     "title": "...",
//     "description": "...",
//     "url": "https://linear.app/...",
//     "team": { "key": "ENG" }
//   }
// }
//
// Signature: HMAC-SHA256 in `linear-signature` header (hex digest, no prefix).
// Secret: LINEAR_WEBHOOK_SECRET env.
pub async fn linear_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> StatusCode {
    let secret = std::env::var("LINEAR_WEBHOOK_SECRET").ok();
    let signature = headers.get("linear-signature").and_then(|v| v.to_str().ok());
    if !verify_raw_hmac(secret.as_deref(), signature, body.as_bytes()) {
        warn!("Linear webhook signature verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let payload: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let action = payload["action"].as_str().unwrap_or("");
    let entity_type = payload["type"].as_str().unwrap_or("");
    if action != "create" || entity_type != "Issue" {
        // Only ingest new issues for now — updates would require status mirroring.
        return StatusCode::OK;
    }

    let data = match payload.get("data") {
        Some(d) => d,
        None => return StatusCode::BAD_REQUEST,
    };

    let external_id = data["id"].as_str().unwrap_or("");
    if external_id.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    if let Ok(Some(_)) = state.db.find_task_by_external("linear", external_id).await {
        info!(external_id, "Linear: task already exists, skipping");
        return StatusCode::OK;
    }

    let identifier = data["identifier"].as_str().unwrap_or("");
    let raw_title = data["title"].as_str().unwrap_or("Untitled");
    let title = if identifier.is_empty() {
        raw_title.to_string()
    } else {
        format!("{}: {}", identifier, raw_title)
    };
    let description = data["description"].as_str().unwrap_or("");
    let url = data["url"].as_str().unwrap_or("");
    let priority = linear_priority(data["priority"].as_i64());

    let user_id = match state.db.first_user_id().await {
        Ok(Some(id)) => id,
        _ => {
            warn!("Linear webhook: no user found to assign task");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    match state
        .db
        .create_task_full(
            user_id,
            &title,
            if description.is_empty() { None } else { Some(description) },
            None, // repo — let the agent figure it out
            priority,
            None,
            None,
            None, // human assignee — default to agent
            None, // assigned_agent_id — default profile (any agent)
            Some("linear"),
            Some(external_id),
            if url.is_empty() { None } else { Some(url) },
        )
        .await
    {
        Ok(task) => {
            info!(task_id = %task.id, external_id, "Linear: task ingested");
            let _ = state
                .db
                .insert_log(
                    task.id,
                    "webhook",
                    &format!("Ingested from Linear: {}", url),
                    "info",
                    None,
                )
                .await;
            StatusCode::OK
        }
        Err(e) => {
            warn!(error = %e, "Linear: failed to create task");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn linear_priority(p: Option<i64>) -> &'static str {
    // Linear priority: 0 none, 1 urgent, 2 high, 3 medium, 4 low
    match p.unwrap_or(0) {
        1 => "urgent",
        2 => "high",
        4 => "low",
        _ => "medium",
    }
}

// --- ClickUp ---
//
// ClickUp webhook signs each request with HMAC-SHA256 of the body using the
// webhook's secret, sent in `x-signature` header (raw hex).
//
// Payload for `taskCreated` is sparse (typically just task_id + history_items).
// When CLICKUP_API_TOKEN is configured we fetch the full task to enrich title,
// description, priority, and URL. Without it we fall back to whatever the
// payload contains, finally to a "ClickUp task <id>" placeholder.
pub async fn clickup_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> StatusCode {
    let secret = std::env::var("CLICKUP_WEBHOOK_SECRET").ok();
    let signature = headers.get("x-signature").and_then(|v| v.to_str().ok());
    if !verify_raw_hmac(secret.as_deref(), signature, body.as_bytes()) {
        warn!("ClickUp webhook signature verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let payload: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let event = payload["event"].as_str().unwrap_or("");
    if event != "taskCreated" {
        return StatusCode::OK;
    }

    let task_id = payload["task_id"].as_str().unwrap_or("");
    if task_id.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    if let Ok(Some(_)) = state.db.find_task_by_external("clickup", task_id).await {
        info!(task_id, "ClickUp: task already exists, skipping");
        return StatusCode::OK;
    }

    // Prefer enriched details from the API; fall back to webhook payload.
    let fetched = match fetch_clickup_task(task_id).await {
        Ok(Some(t)) => Some(t),
        Ok(None) => None, // no API token configured
        Err(e) => {
            warn!(task_id, error = %e, "ClickUp: enrichment failed, falling back to payload");
            None
        }
    };

    let payload_title = payload
        .pointer("/task/name")
        .and_then(|v| v.as_str())
        .or_else(|| {
            payload["history_items"]
                .as_array()
                .and_then(|items| items.iter().find(|h| h["field"] == "name"))
                .and_then(|h| h["after"].as_str())
        });

    let title = fetched
        .as_ref()
        .and_then(|t| t["name"].as_str())
        .or(payload_title)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("ClickUp task {}", task_id));

    let description = fetched.as_ref().and_then(|t| {
        // ClickUp returns markdown_description on most endpoints; fall back to text_content.
        t["markdown_description"]
            .as_str()
            .or_else(|| t["description"].as_str())
            .or_else(|| t["text_content"].as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    });

    let external_url = fetched
        .as_ref()
        .and_then(|t| t["url"].as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://app.clickup.com/t/{}", task_id));

    let priority = fetched
        .as_ref()
        .and_then(|t| t["priority"].pointer("/priority"))
        .and_then(|v| v.as_str())
        .map(clickup_priority)
        .unwrap_or("medium");

    let user_id = match state.db.first_user_id().await {
        Ok(Some(id)) => id,
        _ => {
            warn!("ClickUp webhook: no user found to assign task");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    match state
        .db
        .create_task_full(
            user_id,
            &title,
            description.as_deref(),
            None,
            priority,
            None,
            None,
            None, // human assignee — default to agent
            None, // assigned_agent_id — default profile (any agent)
            Some("clickup"),
            Some(task_id),
            Some(&external_url),
        )
        .await
    {
        Ok(task) => {
            info!(task_id_db = %task.id, task_id, enriched = fetched.is_some(), "ClickUp: task ingested");
            let _ = state
                .db
                .insert_log(
                    task.id,
                    "webhook",
                    &format!("Ingested from ClickUp: {}", external_url),
                    "info",
                    None,
                )
                .await;
            StatusCode::OK
        }
        Err(e) => {
            warn!(error = %e, "ClickUp: failed to create task");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Fetch a full task from ClickUp. Returns Ok(None) when CLICKUP_API_TOKEN is
/// not configured, Ok(Some) on success, Err on transport or API failure.
async fn fetch_clickup_task(task_id: &str) -> anyhow::Result<Option<serde_json::Value>> {
    let Ok(api_token) = std::env::var("CLICKUP_API_TOKEN") else {
        return Ok(None);
    };

    let url = format!("https://api.clickup.com/api/v2/task/{task_id}?include_markdown_description=true");
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", api_token)
        .header("Accept", "application/json")
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("ClickUp API returned {status}: {text}");
    }
    Ok(Some(resp.json::<serde_json::Value>().await?))
}

fn clickup_priority(p: &str) -> &'static str {
    // ClickUp priority strings (case-insensitive): "urgent", "high", "normal", "low"
    match p.to_ascii_lowercase().as_str() {
        "urgent" => "urgent",
        "high" => "high",
        "low" => "low",
        _ => "medium",
    }
}
