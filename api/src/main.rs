use std::net::SocketAddr;
use std::time::Duration;

use axum::http::{self, Method};
use axum::{middleware, routing::{delete, get, patch, post}, Router};
use tower_http::cors::CorsLayer;
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

    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let frontend_url = frontend_url.trim_end_matches('/').to_string();

    let db = karna_shared::db::Database::connect(&database_url).await?;
    let redis = redis::Client::open(redis_url)?;
    let config = config::load()?;

    let state = AppState { db, redis, config };

    let origin = frontend_url
        .parse::<http::HeaderValue>()
        .expect("FRONTEND_URL must be a valid header value");

    let cors = CorsLayer::new()
        .allow_origin(origin)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([
            http::header::CONTENT_TYPE,
            http::header::AUTHORIZATION,
        ])
        .allow_credentials(true)
        .max_age(Duration::from_secs(3600));

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
        .route("/repos/{id}", delete(routes::repos::delete))
        .route("/repos/{id}/onboard", post(routes::repos::trigger_onboard))
        // Config
        .route("/config", get(routes::config::get))
        // Auth middleware on all routes
        .layer(middleware::from_fn(auth::auth_middleware));

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/webhooks/github", post(routes::webhooks::github_webhook))
        .nest("/api", api)
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("API server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
