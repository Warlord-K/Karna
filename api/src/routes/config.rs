use axum::{extract::State, http::StatusCode, Extension, Json};
use serde_json::{json, Value};

use crate::auth::UserId;
use crate::AppState;

pub async fn get(
    State(state): State<AppState>,
    Extension(_user): Extension<UserId>,
) -> Result<Json<Value>, StatusCode> {
    let config = &state.config;

    // Start with config repos
    let mut seen = std::collections::HashSet::new();
    let mut repos: Vec<Value> = config
        .repos
        .iter()
        .map(|r| {
            seen.insert(r.repo.clone());
            json!({
                "repo": r.repo,
                "name": r.repo.split('/').last().unwrap_or(&r.repo),
                "branch": r.branch,
            })
        })
        .collect();

    // Merge in DB repos (added via UI) that aren't already in config
    if let Ok(db_profiles) = state.db.get_all_repo_profiles().await {
        for p in db_profiles {
            if seen.insert(p.repo.clone()) {
                repos.push(json!({
                    "repo": p.repo,
                    "name": p.repo.split('/').last().unwrap_or(&p.repo),
                    "branch": p.branch,
                }));
            }
        }
    }

    let mut backends = serde_json::Map::new();
    for (name, cfg) in &config.backends {
        backends.insert(
            name.clone(),
            json!({
                "models": cfg.models,
                "default_model": cfg.default_model.as_deref().unwrap_or(cfg.models.first().map(|s| s.as_str()).unwrap_or("")),
            }),
        );
    }

    // Fallback
    if backends.is_empty() {
        backends.insert(
            "claude".to_string(),
            json!({ "models": ["haiku", "sonnet", "opus"], "default_model": "sonnet" }),
        );
    }

    let skills: Vec<&str> = config.skills.iter().map(|s| s.name.as_str()).collect();
    let mcp_servers: Vec<&str> = config.mcp_servers.iter().map(|s| s.name.as_str()).collect();

    Ok(Json(json!({
        "repos": repos,
        "backends": backends,
        "skills": skills,
        "mcpServers": mcp_servers,
    })))
}
