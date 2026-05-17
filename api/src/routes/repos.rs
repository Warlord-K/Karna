use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use karna_shared::cache;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::UserId;
use crate::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
) -> Result<Json<Vec<karna_shared::models::RepoProfile>>, StatusCode> {
    let db = state.db.clone();
    let repos = cache::get_or_set(
        &state.redis,
        cache::REPOS_LIST_KEY,
        cache::DEFAULT_TTL_SECS,
        move || async move { db.get_all_repo_profiles().await },
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(repos))
}

#[derive(Deserialize)]
pub struct AddRepo {
    repo: String,
    branch: Option<String>,
}

pub async fn add(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(body): Json<AddRepo>,
) -> Result<(StatusCode, Json<karna_shared::models::RepoProfile>), StatusCode> {
    if body.repo.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let profile = state
        .db
        .upsert_repo_profile(user.0, body.repo.trim(), body.branch.as_deref().unwrap_or("main"))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(profile)))
}

#[derive(Deserialize)]
pub struct UpdateRepo {
    sync_issues: Option<bool>,
    review_prs: Option<bool>,
    /// Omitted = unchanged; explicit `null` = clear; UUID string = set.
    /// Modeled as raw JSON so we can distinguish "absent" from "null".
    #[serde(default)]
    review_agent_id: Option<Value>,
}

pub async fn update(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateRepo>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(sync_issues) = body.sync_issues {
        state
            .db
            .update_repo_sync_issues(id, sync_issues)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let review_agent_change: Option<Option<Uuid>> = match body.review_agent_id {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) => match Uuid::parse_str(&s) {
            Ok(u) => Some(Some(u)),
            Err(_) => return Err(StatusCode::BAD_REQUEST),
        },
        Some(_) => return Err(StatusCode::BAD_REQUEST),
    };

    if body.review_prs.is_some() || review_agent_change.is_some() {
        state
            .db
            .update_repo_review_config(id, body.review_prs, review_agent_change)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Return updated profile
    let profiles = state
        .db
        .get_all_repo_profiles()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let profile = profiles.into_iter().find(|p| p.id == id);
    match profile {
        Some(p) => Ok(Json(json!(p))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    state
        .db
        .delete_repo_profile(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

/// Reset `webhook_status` so the agent's reconciler picks up this repo on
/// its next poll cycle (typically within seconds). The frontend polls the
/// repos list and sees the status change.
pub async fn trigger_webhook_register(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    state
        .db
        .set_repo_webhook_status(id, "not_registered", None, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "message": "Webhook re-registration queued" })))
}

pub async fn list_reviews(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<karna_shared::models::PrReview>>, StatusCode> {
    let profiles = state
        .db
        .get_all_repo_profiles()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let repo = profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or(StatusCode::NOT_FOUND)?
        .repo;

    let db = state.db.clone();
    let cache_key = cache::pr_reviews_for_repo_key(id);
    let reviews = cache::get_or_set(&state.redis, &cache_key, 60, move || async move {
        db.list_pr_reviews_for_repo(&repo, 50).await
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(reviews))
}

pub async fn review_logs(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
    Path((_repo_id, review_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<karna_shared::models::PrReviewLog>>, StatusCode> {
    // Ownership scope: the review must exist. We don't tie it back to the
    // repo_id beyond URL routing — the review row carries the repo string.
    if state.db.get_pr_review(review_id).await.ok().flatten().is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let db = state.db.clone();
    let cache_key = cache::pr_review_logs_key(review_id);
    let logs = cache::get_or_set(&state.redis, &cache_key, 5, move || async move {
        db.get_pr_review_logs(review_id, 200).await
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(logs))
}

pub async fn trigger_onboard(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    state
        .db
        .set_repo_profile_status(id, "pending")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}
