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
    let branch = payload
        .pointer("/pull_request/head/ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Only handle agent branches ({prefix}-{number}/slug format)
    // Skip branches that don't match the pattern (e.g., "main", "feature/foo")
    if !branch.contains('/') || !branch.split('/').next().map_or(false, |p| {
        p.rfind('-').map_or(false, |i| p[i + 1..].chars().all(|c| c.is_ascii_digit()) && i > 0)
    }) {
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

fn verify_signature(secret: Option<&str>, signature_header: Option<&str>, body: &[u8]) -> bool {
    let secret = match secret {
        Some(s) => s,
        None => {
            warn!("GITHUB_WEBHOOK_SECRET not configured — rejecting webhook");
            return false;
        }
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
