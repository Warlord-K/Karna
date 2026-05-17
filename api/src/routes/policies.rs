use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use karna_shared::{cache, models::Policy};

use crate::AppState;

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<Policy>>, StatusCode> {
    let db = state.db.clone();
    let rows = cache::get_or_set(
        &state.redis,
        cache::POLICIES_LIST_KEY,
        cache::DEFAULT_TTL_SECS,
        move || async move { db.list_policies().await },
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct CreatePolicy {
    name: String,
    repo_pattern: Option<String>,
    path_glob: String,
    message: String,
    severity: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreatePolicy>,
) -> Result<(StatusCode, Json<Policy>), StatusCode> {
    let name = body.name.trim();
    let path_glob = body.path_glob.trim();
    let message = body.message.trim();
    if name.is_empty() || path_glob.is_empty() || message.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let severity = body.severity.as_deref().unwrap_or("warn");
    if !matches!(severity, "warn" | "block") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let repo_pattern = body.repo_pattern.as_deref().unwrap_or("*").trim();
    let repo_pattern = if repo_pattern.is_empty() { "*" } else { repo_pattern };

    let row = state
        .db
        .create_policy(name, repo_pattern, path_glob, message, severity)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let rows = state
        .db
        .update_policy(id, &body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if rows == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let rows = state
        .db
        .delete_policy(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if rows == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!({ "ok": true })))
}
