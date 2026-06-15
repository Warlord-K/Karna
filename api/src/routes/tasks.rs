use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashSet, convert::Infallible, time::Duration};
use tracing::warn;
use uuid::Uuid;

use karna_shared::cache;
use karna_shared::db::LogCursor;
use karna_shared::models::{AgentTask, OrchestratorConfig, TaskKind, TaskOutputTarget};

use crate::auth::UserId;
use crate::AppState;

const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Deserialize)]
pub struct CreateTask {
    title: String,
    description: Option<String>,
    repo: Option<String>,
    priority: Option<String>,
    cli: Option<String>,
    model: Option<String>,
    /// Task kind. Defaults to `code` when omitted.
    kind: Option<String>,
    /// Artifact delivery target for non-code flows. Defaults to `none`.
    output_target: Option<String>,
    /// User UUID. NULL = pick up by agent (existing behavior).
    assignee_user_id: Option<Uuid>,
    /// Agent profile UUID. NULL = any agent picks it up.
    /// Mutually exclusive with assignee_user_id at the frontend, but the DB
    /// accepts either independently so callers can model intent precisely.
    assigned_agent_id: Option<Uuid>,
    /// Per-stage agent profile overrides for the multi-agent flow. Each is an
    /// agent profile UUID; NULL falls back to assigned_agent_id then default.
    planner_agent_id: Option<Uuid>,
    implementer_agent_id: Option<Uuid>,
    reviewer_agent_id: Option<Uuid>,
    /// "linear" | "clickup" — only set when ingesting from an external system.
    external_source: Option<String>,
    external_id: Option<String>,
    external_url: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateOrchestratorTask {
    title: String,
    description: Option<String>,
    repo: Option<String>,
    priority: Option<String>,
    source: Option<String>,
    slack_channel: Option<String>,
    thread_ts: Option<String>,
    orchestrator: Option<OrchestratorConfig>,
}

pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Vec<karna_shared::models::AgentTask>>, StatusCode> {
    let key = cache::tasks_list_key(user.0);
    let db = state.db.clone();
    let tasks = cache::get_or_set(
        &state.redis,
        &key,
        cache::DEFAULT_TTL_SECS,
        move || async move { db.list_tasks_for_user(user.0).await },
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(tasks))
}

pub async fn list_chats(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Vec<karna_shared::models::AgentTask>>, StatusCode> {
    let chats = state
        .db
        .list_chats_for_user(user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(chats))
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
    let kind = body
        .kind
        .as_deref()
        .unwrap_or(TaskKind::Code.as_str())
        .parse::<TaskKind>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let output_target = body
        .output_target
        .as_deref()
        .unwrap_or(TaskOutputTarget::None.as_str())
        .parse::<TaskOutputTarget>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let task = state
        .db
        .create_task_full(
            user.0,
            title,
            body.description.as_deref(),
            body.repo.as_deref(),
            body.priority.as_deref().unwrap_or("medium"),
            body.cli.as_deref(),
            body.model.as_deref(),
            body.assignee_user_id,
            body.assigned_agent_id,
            body.external_source.as_deref(),
            body.external_id.as_deref(),
            body.external_url.as_deref(),
            body.planner_agent_id,
            body.implementer_agent_id,
            body.reviewer_agent_id,
            Some(kind.as_str()),
            Some(output_target.as_str()),
            None,
            None,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(task)))
}

pub async fn create_orchestrator_task(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(body): Json<CreateOrchestratorTask>,
) -> Result<(StatusCode, Json<karna_shared::models::AgentTask>), StatusCode> {
    let title = body.title.trim();
    if title.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.thread_ts.is_some() && body.slack_channel.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let source = match body.source.as_deref() {
        None => None,
        Some("chat") => Some("chat"),
        Some(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let mut orchestrator_cfg = body.orchestrator.unwrap_or_default();
    let configured_chat_tools = sanitize_allowed_tools(
        state
            .config
            .mcp_servers
            .iter()
            .map(|server| server.name.clone())
            .collect(),
    );
    if source == Some("chat") {
        orchestrator_cfg.allowed_tools =
            resolve_chat_allowed_tools(orchestrator_cfg.allowed_tools, configured_chat_tools);
        orchestrator_cfg.accepts_external_replies = false;
    } else {
        orchestrator_cfg.allowed_tools = sanitize_allowed_tools(orchestrator_cfg.allowed_tools);
    }
    let orchestrator_json =
        serde_json::to_value(orchestrator_cfg).map_err(|_| StatusCode::BAD_REQUEST)?;

    let task = state
        .db
        .create_task_full(
            user.0,
            title,
            body.description.as_deref(),
            body.repo.as_deref(),
            body.priority.as_deref().unwrap_or("medium"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(TaskKind::Ops.as_str()),
            Some(TaskOutputTarget::None.as_str()),
            Some(&orchestrator_json),
            source,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut updates = std::collections::HashMap::new();
    if let Some(channel) = body.slack_channel.as_deref() {
        updates.insert(
            "slack_channel".to_string(),
            Value::String(channel.to_string()),
        );
        if let Some(thread_ts) = body.thread_ts.as_deref() {
            updates.insert(
                "slack_thread_ts".to_string(),
                Value::String(thread_ts.to_string()),
            );
        }
    }
    if !updates.is_empty() {
        let _ = state
            .db
            .update_task(task.id, user.0, &updates)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let task = state
        .db
        .get_task(task.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

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
    if !state
        .db
        .task_belongs_to_user(id, user.0)
        .await
        .unwrap_or(false)
    {
        return Err(StatusCode::NOT_FOUND);
    }

    let key = cache::tasks_logs_key(id);
    let db = state.db.clone();
    let logs = cache::get_or_set(
        &state.redis,
        &key,
        cache::DEFAULT_TTL_SECS,
        move || async move { db.get_logs(id, 200).await },
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(logs))
}

#[derive(Deserialize)]
pub struct LogsStreamQuery {
    /// Optional cursor in the form "<rfc3339>|<uuid>".
    after: Option<String>,
}

pub async fn logs_stream(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<Uuid>,
    Query(query): Query<LogsStreamQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    if !state
        .db
        .task_belongs_to_user(id, user.0)
        .await
        .unwrap_or(false)
    {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut cursor = match query.after {
        Some(raw) => Some(parse_log_cursor(&raw).map_err(|_| StatusCode::BAD_REQUEST)?),
        None => None,
    };
    let db = state.db.clone();

    let stream = async_stream::stream! {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tick.tick().await;
            match db.get_logs_since(id, cursor.clone(), 200).await {
                Ok(logs) => {
                    for log in logs {
                        if let Some(created_at) = log.created_at {
                            cursor = Some(LogCursor {
                                created_at,
                                id: log.id,
                            });
                        }

                        match serde_json::to_string(&log) {
                            Ok(payload) => {
                                yield Ok::<Event, Infallible>(
                                    Event::default()
                                        .event("log")
                                        .id(log.id.to_string())
                                        .data(payload),
                                );
                            }
                            Err(error) => {
                                warn!(task_id = %id, %error, "failed to serialize task log for SSE");
                            }
                        }
                    }
                }
                Err(error) => {
                    warn!(task_id = %id, %error, "failed to poll task logs for SSE");
                }
            }
        }
    };

    let sse = Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    );
    Ok((
        [("cache-control", "no-cache"), ("x-accel-buffering", "no")],
        sse,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorParseError {
    Format,
    Timestamp,
    Uuid,
}

fn parse_log_cursor(raw: &str) -> Result<LogCursor, CursorParseError> {
    let (created_at_raw, id_raw) = raw.split_once('|').ok_or(CursorParseError::Format)?;
    let created_at = DateTime::parse_from_rfc3339(created_at_raw)
        .map_err(|_| CursorParseError::Timestamp)?
        .with_timezone(&Utc);
    let id = Uuid::parse_str(id_raw).map_err(|_| CursorParseError::Uuid)?;
    Ok(LogCursor { created_at, id })
}

fn resolve_chat_allowed_tools(
    requested_tools: Vec<String>,
    configured_tools: Vec<String>,
) -> Vec<String> {
    let requested = sanitize_allowed_tools(requested_tools);
    if requested.is_empty() {
        configured_tools
    } else {
        requested
    }
}

fn sanitize_allowed_tools(tools: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut sanitized = Vec::new();
    for raw in tools {
        let tool = raw.trim();
        if tool.is_empty() {
            continue;
        }
        if seen.insert(tool.to_string()) {
            sanitized.push(tool.to_string());
        }
    }
    sanitized
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

    let default_id = Uuid::parse_str(DEFAULT_USER_ID).unwrap();
    if !state.db.is_shared_workspace() && task.user_id != user.0 && task.user_id != default_id {
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
    if !state
        .db
        .task_belongs_to_user(id, user.0)
        .await
        .unwrap_or(false)
    {
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

    let default_id = Uuid::parse_str(DEFAULT_USER_ID).unwrap();
    if !state.db.is_shared_workspace() && task.user_id != user.0 && task.user_id != default_id {
        return Err(StatusCode::NOT_FOUND);
    }

    if task.status != "plan_review" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let plan = task
        .plan_content
        .as_deref()
        .ok_or(StatusCode::BAD_REQUEST)?;

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
                task.user_id,
                &def.title,
                def.description.as_deref(),
                Some(&def.repo),
                &task.priority,
                task.cli.as_deref(),
                task.model.as_deref(),
                None,
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let _ = state
            .db
            .insert_log(
                task.id,
                "plan",
                &format!("Created subtask: {} ({})", sub.title, sub.id),
                "info",
                Some(task_card_metadata(&sub)),
            )
            .await;
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

fn task_card_metadata(task: &AgentTask) -> Value {
    let mut metadata = serde_json::Map::new();
    metadata.insert("card".to_string(), json!("task"));
    metadata.insert("task_id".to_string(), json!(task.id));
    metadata.insert("title".to_string(), json!(task.title));
    metadata.insert("status".to_string(), json!(task.status));
    if let Some(number) = task.task_number {
        metadata.insert("task_number".to_string(), json!(number));
    }
    if !task.kind.trim().is_empty() {
        metadata.insert("kind".to_string(), json!(task.kind));
    }
    Value::Object(metadata)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_log_cursor, resolve_chat_allowed_tools, sanitize_allowed_tools, CursorParseError,
    };
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    #[test]
    fn parses_valid_log_cursor() {
        let id = Uuid::new_v4();
        let cursor = format!("2026-06-14T16:30:00Z|{id}");
        let parsed = parse_log_cursor(&cursor).expect("cursor should parse");
        assert_eq!(
            parsed.created_at,
            Utc.with_ymd_and_hms(2026, 6, 14, 16, 30, 0).unwrap()
        );
        assert_eq!(parsed.id, id);
    }

    #[test]
    fn rejects_missing_delimiter() {
        let err = parse_log_cursor("2026-06-14T16:30:00Z").expect_err("cursor should be rejected");
        assert_eq!(err, CursorParseError::Format);
    }

    #[test]
    fn rejects_invalid_timestamp() {
        let err = parse_log_cursor("not-a-time|00000000-0000-0000-0000-000000000000")
            .expect_err("cursor should be rejected");
        assert_eq!(err, CursorParseError::Timestamp);
    }

    #[test]
    fn rejects_invalid_uuid() {
        let err = parse_log_cursor("2026-06-14T16:30:00Z|not-a-uuid")
            .expect_err("cursor should be rejected");
        assert_eq!(err, CursorParseError::Uuid);
    }

    #[test]
    fn sanitizes_allowed_tools() {
        let sanitized = sanitize_allowed_tools(vec![
            "  ".to_string(),
            "context7".to_string(),
            "context7".to_string(),
            " github  ".to_string(),
        ]);
        assert_eq!(sanitized, vec!["context7", "github"]);
    }

    #[test]
    fn chat_defaults_to_configured_tools_when_request_is_empty() {
        let allowed = resolve_chat_allowed_tools(
            vec![" ".to_string()],
            vec!["context7".to_string(), "github".to_string()],
        );
        assert_eq!(allowed, vec!["context7", "github"]);
    }

    #[test]
    fn chat_preserves_explicit_requested_tools() {
        let allowed = resolve_chat_allowed_tools(
            vec![" custom/server ".to_string()],
            vec!["context7".to_string()],
        );
        assert_eq!(allowed, vec!["custom/server"]);
    }
}
