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

#[derive(Deserialize)]
pub struct CreateTask {
    title: String,
    description: Option<String>,
    repo: Option<String>,
    priority: Option<String>,
    cli: Option<String>,
    model: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Vec<karna_shared::models::AgentTask>>, StatusCode> {
    let tasks = state
        .db
        .list_tasks_for_user(user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(tasks))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(body): Json<CreateTask>,
) -> Result<(StatusCode, Json<karna_shared::models::AgentTask>), StatusCode> {
    let title = body.title.trim();
    if title.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let task = state
        .db
        .create_task(
            user.0,
            title,
            body.description.as_deref(),
            body.repo.as_deref(),
            body.priority.as_deref().unwrap_or("medium"),
            body.cli.as_deref(),
            body.model.as_deref(),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(task)))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
    Json(body): Json<std::collections::HashMap<String, Value>>,
) -> Result<Json<Value>, StatusCode> {
    let rows = state
        .db
        .update_task(id, user.0, &body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if rows == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(json!({ "ok": true })))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let rows = state
        .db
        .delete_task(id, user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if rows == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(json!({ "ok": true })))
}

pub async fn logs(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<karna_shared::models::AgentLog>>, StatusCode> {
    if !state.db.task_belongs_to_user(id, user.0).await.unwrap_or(false) {
        return Err(StatusCode::NOT_FOUND);
    }

    let logs = state
        .db
        .get_logs(id, 200)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(logs))
}

#[derive(Deserialize)]
pub struct CommentBody {
    message: String,
}

pub async fn post_comment(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
    Json(body): Json<CommentBody>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    let task = state
        .db
        .get_task(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if task.user_id != user.0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Insert comment log
    state
        .db
        .insert_log(id, "user", &body.message, "comment", None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Set feedback for agent to pick up
    state
        .db
        .set_feedback(id, &body.message)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Transition review → in_progress or plan_review → planning
    let new_status = match task.status.as_str() {
        "review" => Some("in_progress"),
        "plan_review" => Some("planning"),
        _ => None,
    };
    if let Some(status) = new_status {
        let _ = state.db.update_status(id, status).await;
    }

    Ok((StatusCode::CREATED, Json(json!({ "ok": true }))))
}

pub async fn list_subtasks(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<karna_shared::models::AgentTask>>, StatusCode> {
    if !state.db.task_belongs_to_user(id, user.0).await.unwrap_or(false) {
        return Err(StatusCode::NOT_FOUND);
    }

    let subtasks = state
        .db
        .get_subtasks(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(subtasks))
}

/// Parse subtask definitions from plan_content and create child tasks.
pub async fn create_subtasks(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<Vec<karna_shared::models::AgentTask>>), StatusCode> {
    let task = state
        .db
        .get_task(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if task.user_id != user.0 {
        return Err(StatusCode::NOT_FOUND);
    }

    if task.status != "plan_review" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let plan = task.plan_content.as_deref().ok_or(StatusCode::BAD_REQUEST)?;

    // Parse <!-- subtasks [...] subtasks --> block
    let re = regex_lite::Regex::new(r"<!--\s*subtasks\s*\n([\s\S]*?)\nsubtasks\s*-->").unwrap();
    let caps = re.captures(plan).ok_or(StatusCode::BAD_REQUEST)?;
    let json_str = &caps[1];

    let defs: Vec<SubtaskDef> =
        serde_json::from_str(json_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    if defs.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check idempotency
    let existing = state.db.get_subtasks(id).await.unwrap_or_default();
    if !existing.is_empty() {
        return Err(StatusCode::CONFLICT);
    }

    let mut created = Vec::new();
    for def in &defs {
        if def.title.is_empty() || def.repo.is_empty() {
            continue;
        }
        let sub = state
            .db
            .create_subtask(
                id,
                user.0,
                &def.title,
                def.description.as_deref(),
                &def.repo,
                &task.priority,
                task.cli.as_deref(),
                task.model.as_deref(),
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        created.push(sub);
    }

    // Copy parent task attachments to each subtask
    for sub in &created {
        let _ = state.db.copy_task_attachments(id, sub.id).await;
    }

    // Move parent to in_progress
    let _ = state.db.update_status(id, "in_progress").await;

    Ok((StatusCode::CREATED, Json(created)))
}

#[derive(Deserialize)]
struct SubtaskDef {
    title: String,
    repo: String,
    description: Option<String>,
}
