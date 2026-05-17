use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use karna_shared::{
    cache,
    models::{AgentProfile, AgentTask, PrReview},
};

use crate::AppState;

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<AgentProfile>>, StatusCode> {
    let db = state.db.clone();
    let profiles = cache::get_or_set(
        &state.redis,
        cache::AGENTS_LIST_KEY,
        cache::DEFAULT_TTL_SECS,
        move || async move { db.list_agent_profiles().await },
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(profiles))
}

#[derive(Deserialize)]
pub struct CreateAgent {
    slug: String,
    name: String,
    cli: String,
    model: String,
    avatar_emoji: Option<String>,
    system_prompt_addendum: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateAgent>,
) -> Result<(StatusCode, Json<AgentProfile>), StatusCode> {
    let slug = body.slug.trim();
    let name = body.name.trim();
    if slug.is_empty() || name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let profile = state
        .db
        .create_agent_profile(
            slug,
            name,
            body.cli.trim(),
            body.model.trim(),
            body.avatar_emoji.as_deref().unwrap_or("🤖"),
            body.system_prompt_addendum.as_deref(),
        )
        .await
        .map_err(|e| {
            // Likely a UNIQUE constraint violation on slug
            if e.to_string().contains("agent_profiles_slug_key")
                || e.to_string().contains("duplicate key")
            {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;

    Ok((StatusCode::CREATED, Json(profile)))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let rows = state
        .db
        .update_agent_profile(id, &body)
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
        .delete_agent_profile(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if rows == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentProfile>, StatusCode> {
    let profile = state
        .db
        .get_agent_profile(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(profile))
}

#[derive(Serialize)]
pub struct AgentStats {
    pub total_tasks: i64,
    pub open_tasks: i64,
    pub prs_opened: i64,
    pub reviews_done: i64,
    pub cost_usd: f64,
}

pub async fn stats(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentStats>, StatusCode> {
    let (total_tasks, open_tasks, prs_opened, reviews_done, cost_usd) = state
        .db
        .agent_profile_stats(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(AgentStats {
        total_tasks,
        open_tasks,
        prs_opened,
        reviews_done,
        cost_usd,
    }))
}

pub async fn tasks(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<AgentTask>>, StatusCode> {
    let rows = state
        .db
        .list_tasks_for_agent(id, 50)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

pub async fn reviews(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<PrReview>>, StatusCode> {
    let rows = state
        .db
        .list_pr_reviews_by_agent(id, 50)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

/// Unified list of "things a task can be assigned to" — humans + agent profiles —
/// for the frontend's assignee dropdown.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Assignable {
    User {
        id: Uuid,
        name: Option<String>,
        email: Option<String>,
    },
    Agent {
        id: Uuid,
        name: String,
        slug: String,
        avatar_emoji: String,
        paused: bool,
    },
}

pub async fn assignables(
    State(state): State<AppState>,
) -> Result<Json<Vec<Assignable>>, StatusCode> {
    let users = state
        .db
        .list_users()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let agents = state
        .db
        .list_agent_profiles()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut out: Vec<Assignable> = Vec::with_capacity(users.len() + agents.len());
    for a in agents {
        out.push(Assignable::Agent {
            id: a.id,
            name: a.name,
            slug: a.slug,
            avatar_emoji: a.avatar_emoji,
            paused: a.paused_reason.is_some(),
        });
    }
    for (id, name, email) in users {
        out.push(Assignable::User { id, name, email });
    }
    Ok(Json(out))
}
