use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, delete},
    Json, Router,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::Deserialize;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::db::Database;
use crate::models::TaskStatus;

type HmacSha256 = Hmac<Sha256>;

/// Verify the GitHub webhook signature (X-Hub-Signature-256 header).
/// Returns true if no secret is configured (verification disabled).
fn verify_webhook_signature(secret: Option<&str>, signature_header: Option<&str>, body: &[u8]) -> bool {
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
        None => {
            warn!("Webhook signature missing sha256= prefix");
            return false;
        }
    };

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let computed = hex::encode(mac.finalize().into_bytes());

    // Constant-time comparison via the hmac crate isn't exposed through hex,
    // but for webhook verification timing attacks are not practical.
    computed == expected
}

#[derive(Clone)]
struct AppState {
    config: Config,
    db: Database,
}

pub async fn serve(config: Config, db: Database, shutdown: tokio::sync::watch::Receiver<()>) {
    let state = AppState { config, db };

    let app = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/repos", get(list_repos))
        .route("/repos", post(add_repo))
        .route("/repos/{id}", delete(delete_repo))
        .route("/repos/{id}/onboard", post(trigger_onboard))
        .route("/webhooks/github", post(github_webhook))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("API server on :8080");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let mut rx = shutdown;
            let _ = rx.changed().await;
            info!("API server shutting down");
        })
        .await
        .unwrap();
}

async fn health() -> &'static str {
    "ok"
}

async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let has_active = !state.db.active_task_ids().await.unwrap_or_default().is_empty();
    let next = state.db.next_actionable_task().await.ok().flatten();

    Json(serde_json::json!({
        "status": if has_active { "working" } else { "idle" },
        "queue": next.map(|t| serde_json::json!({
            "id": t.id.to_string(),
            "title": t.title,
            "status": t.status,
        })),
    }))
}

// --- Repo profile endpoints ---

async fn list_repos(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let profiles = state.db.get_all_repo_profiles().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!(profiles)))
}

#[derive(Deserialize)]
struct AddRepoRequest {
    repo: String,
    #[serde(default = "default_branch")]
    branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

async fn add_repo(
    State(state): State<AppState>,
    Json(body): Json<AddRepoRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let user_id = state.db.first_user_id().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let profile = state.db
        .upsert_repo_profile(user_id, &body.repo, &body.branch)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(repo = %body.repo, "Repo profile added via API");

    // Spawn async onboarding in the background
    let config = state.config.clone();
    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::onboarding::sync_repo_profiles(&config, &db).await {
            warn!(error = %e, "Background onboarding failed");
        }
    });

    Ok(Json(serde_json::json!(profile)))
}

async fn delete_repo(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> StatusCode {
    match state.db.delete_repo_profile(id).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn trigger_onboard(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> StatusCode {
    // Set status back to pending so sync picks it up
    if state.db.set_repo_profile_status(id, "pending").await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Spawn background onboarding
    let config = state.config.clone();
    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::onboarding::sync_repo_profiles(&config, &db).await {
            warn!(error = %e, "Background onboarding failed");
        }
    });

    StatusCode::ACCEPTED
}

async fn github_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> StatusCode {
    // Verify webhook signature if a secret is configured
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok());
    if !verify_webhook_signature(
        state.config.github_webhook_secret.as_deref(),
        signature,
        body.as_bytes(),
    ) {
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

    let branch = payload
        .pointer("/pull_request/head/ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Only handle agent branches ({prefix}-{number}/slug format)
    if !branch.contains('/') || !branch.split('/').next().is_some_and(|p| {
        p.rfind('-').is_some_and(|i| p[i + 1..].chars().all(|c| c.is_ascii_digit()) && i > 0)
    }) {
        return StatusCode::OK;
    }

    // Look up the task by branch
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
        let _ = state
            .db
            .update_status(task.id, TaskStatus::Done.as_str())
            .await;
        let _ = state
            .db
            .insert_log(task.id, "webhook", "PR merged, task complete", "info", None)
            .await;
        let _ = crate::notifications::send_task_done(&state.config, &task).await;
        return StatusCode::OK;
    }

    // PR review: changes requested → set feedback and move to in_progress
    if action == "submitted" {
        let review_state = payload
            .pointer("/review/state")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if review_state == "changes_requested" {
            let review_body = payload
                .pointer("/review/body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            info!(task_id = %task.id, branch, "Webhook: changes requested");

            if !review_body.is_empty() {
                let _ = state.db.set_feedback(task.id, &review_body).await;
            }
            let _ = state
                .db
                .update_status(task.id, TaskStatus::InProgress.as_str())
                .await;
            let _ = state
                .db
                .insert_log(
                    task.id,
                    "webhook",
                    "Changes requested via PR review",
                    "info",
                    None,
                )
                .await;
            return StatusCode::OK;
        }

        // PR approved → just log it (user still needs to merge)
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
