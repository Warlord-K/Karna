use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::UserId;
use crate::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
) -> Result<Json<Vec<karna_shared::models::RepoProfile>>, StatusCode> {
    let repos = state
        .db
        .get_all_repo_profiles()
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
