use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize)]
pub struct UserSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<UserSummary>>, StatusCode> {
    let rows = state
        .db
        .list_users()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let users = rows
        .into_iter()
        .map(|(id, name, email)| UserSummary { id, name, email })
        .collect();

    Ok(Json(users))
}
