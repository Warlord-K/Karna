use std::net::SocketAddr;

use axum::{middleware, routing::{get, patch, post}, Router};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

mod auth;
mod config;
mod routes;

pub use config::ApiConfig;

#[derive(Clone)]
pub struct AppState {
    pub db: karna_shared::db::Database,
    pub redis: redis::Client,
    pub config: ApiConfig,
}

fn parse_bool_env(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("karna_api=info".parse()?),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse()
        .expect("PORT must be a number");

    let redis = redis::Client::open(redis_url)?;
    let shared_workspace = parse_bool_env("KARNA_SHARED_WORKSPACE");
    if shared_workspace {
        info!("Shared-workspace mode enabled — every signed-in user can see and edit all tasks/schedules");
    }
    let db = karna_shared::db::Database::connect(&database_url)
        .await?
        .with_redis(redis.clone())
        .with_shared_workspace(shared_workspace);
    let config = config::load()?;

    let state = AppState { db, redis, config };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = Router::new()
        // Tasks
        .route("/tasks", get(routes::tasks::list).post(routes::tasks::create))
        .route(
            "/tasks/{id}",
            patch(routes::tasks::update).delete(routes::tasks::delete),
        )
        .route("/tasks/{id}/logs", get(routes::tasks::logs))
        .route("/tasks/{id}/comments", post(routes::tasks::post_comment))
        .route(
            "/tasks/{id}/subtasks",
            get(routes::tasks::list_subtasks).post(routes::tasks::create_subtasks),
        )
        // Schedules
        .route(
            "/schedules",
            get(routes::schedules::list).post(routes::schedules::create),
        )
        .route(
            "/schedules/{id}",
            get(routes::schedules::get)
                .patch(routes::schedules::update)
                .delete(routes::schedules::delete),
        )
        .route("/schedules/{id}/trigger", post(routes::schedules::trigger))
        .route("/schedules/{id}/runs", get(routes::schedules::list_runs))
        .route(
            "/schedules/{id}/runs/{run_id}/logs",
            get(routes::schedules::run_logs),
        )
        // Repos
        .route("/repos", get(routes::repos::list).post(routes::repos::add))
        .route("/repos/{id}", patch(routes::repos::update).delete(routes::repos::delete))
        .route("/repos/{id}/onboard", post(routes::repos::trigger_onboard))
        // Users (for assignee dropdown)
        .route("/users", get(routes::users::list))
        // Agent profiles (pseudo-users)
        .route("/agents", get(routes::agents::list).post(routes::agents::create))
        .route(
            "/agents/{id}",
            patch(routes::agents::update).delete(routes::agents::delete),
        )
        // Unified assignee picker (humans + agents)
        .route("/assignables", get(routes::agents::assignables))
        // Config
        .route("/config", get(routes::config::get))
        // Auth middleware on all routes
        .layer(middleware::from_fn(auth::auth_middleware));

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/webhooks/github", post(routes::webhooks::github_webhook))
        .route("/webhooks/linear", post(routes::webhooks::linear_webhook))
        .route("/webhooks/clickup", post(routes::webhooks::clickup_webhook))
        .nest("/api", api)
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("API server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
