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
pub struct CreateSchedule {
    name: String,
    prompt: String,
    repos: Option<String>,
    cron_expression: Option<String>,
    run_at: Option<String>,
    skills: Option<Vec<String>>,
    mcp_servers: Option<Vec<String>>,
    max_open_tasks: Option<i32>,
    task_prefix: Option<String>,
    priority: Option<String>,
    cli: Option<String>,
    model: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, StatusCode> {
    let schedules = state
        .db
        .list_schedules_for_user(user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Join last run for each schedule
    let mut result = Vec::new();
    for s in &schedules {
        let last_run = state.db.get_last_run(s.id).await.ok().flatten();
        let mut val = serde_json::to_value(s).unwrap_or(json!({}));
        val["last_run"] = serde_json::to_value(&last_run).unwrap_or(Value::Null);
        result.push(val);
    }

    Ok(Json(Value::Array(result)))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(body): Json<CreateSchedule>,
) -> Result<(StatusCode, Json<karna_shared::models::Schedule>), StatusCode> {
    if body.name.trim().is_empty() || body.prompt.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let schedule = state
        .db
        .create_schedule(
            user.0,
            body.name.trim(),
            body.prompt.trim(),
            body.repos.as_deref(),
            body.cron_expression.as_deref(),
            body.run_at.as_deref(),
            &body.skills.unwrap_or_default(),
            &body.mcp_servers.unwrap_or_default(),
            body.max_open_tasks.unwrap_or(5),
            body.task_prefix.as_deref(),
            body.priority.as_deref().unwrap_or("medium"),
            body.cli.as_deref(),
            body.model.as_deref(),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(schedule)))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    if !state.db.schedule_belongs_to_user(id, user.0).await.unwrap_or(false) {
        return Err(StatusCode::NOT_FOUND);
    }

    let schedule = state
        .db
        .get_schedule(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let last_run = state.db.get_last_run(id).await.ok().flatten();
    let mut val = serde_json::to_value(&schedule).unwrap_or(json!({}));
    val["last_run"] = serde_json::to_value(&last_run).unwrap_or(Value::Null);

    Ok(Json(val))
}

pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let rows = state
        .db
        .update_schedule_fields(id, user.0, &body)
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
        .delete_schedule(id, user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if rows == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(json!({ "ok": true })))
}

pub async fn trigger(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    if !state.db.schedule_belongs_to_user(id, user.0).await.unwrap_or(false) {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut conn = state
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    redis::cmd("SET")
        .arg(format!("schedule_trigger:{id}"))
        .arg("1")
        .arg("EX")
        .arg(300)
        .exec_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "ok": true, "message": "Schedule triggered" })))
}

pub async fn list_runs(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<karna_shared::models::ScheduledRun>>, StatusCode> {
    if !state.db.schedule_belongs_to_user(id, user.0).await.unwrap_or(false) {
        return Err(StatusCode::NOT_FOUND);
    }

    let runs = state
        .db
        .get_schedule_runs(id, 50)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(runs))
}

pub async fn run_logs(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<karna_shared::models::ScheduledRunLog>>, StatusCode> {
    if !state.db.schedule_belongs_to_user(id, user.0).await.unwrap_or(false) {
        return Err(StatusCode::NOT_FOUND);
    }

    let logs = state
        .db
        .get_run_logs(run_id, 200)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(logs))
}
