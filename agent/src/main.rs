use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{error, info, warn};

mod agent;
mod api;
mod claude;
mod cli;
mod codex;
mod config;
mod db;
mod git;
mod models;
mod notifications;
mod onboarding;
mod queue;
mod scheduler;
mod updater;

/// Exit code indicating the agent needs a rebuild (code changes detected).
pub const EXIT_CODE_UPDATE: i32 = 42;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("karna_agent=info".parse()?),
        )
        .json()
        .init();

    dotenvy::dotenv().ok();

    let mut config = config::Config::from_env()?;
    let backend_names: Vec<&str> = config.backends.keys().map(|s| s.as_str()).collect();
    info!(
        repos = config.repos.len(),
        backends = ?backend_names,
        poll_interval = config.poll_interval_secs,
        "Karna Agent starting",
    );

    // Graceful shutdown — flag + notify for instant wake from sleep
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    // Watch channel to signal the API server to stop accepting connections
    let (api_shutdown_tx, api_shutdown_rx) = tokio::sync::watch::channel(());

    // Ensure directories exist
    tokio::fs::create_dir_all(&config.repos_dir).await?;
    tokio::fs::create_dir_all(&config.workspaces_dir).await?;

    // Configure git credential store for HTTPS auth
    git::workspace::configure_git_auth(&config.github_token).await?;

    // Configure SSH commit signing (auto-detected from ./signing/ mount or explicit config)
    git::workspace::configure_git_signing(config.signing.as_ref()).await?;

    // Connect to Postgres
    let db = db::Database::connect(&config.database_url).await?;
    info!("Connected to Postgres");

    // Connect to Redis
    let redis = redis::Client::open(config.redis_url.as_str())?;
    // Verify connection
    let mut conn = redis.get_multiplexed_async_connection().await?;
    redis::cmd("PING")
        .query_async::<String>(&mut conn)
        .await?;
    info!("Connected to Redis");

    // Sync config-defined schedules to database
    match scheduler::sync_config_schedules(&config, &db).await {
        Ok(()) => {}
        Err(e) => warn!(error = %e, "Failed to sync config schedules"),
    }

    // Clone repos on startup
    for repo in &config.repos {
        info!(repo = %repo.repo, "Ensuring repo is cloned");
        match git::workspace::ensure_cloned(&config.repos_dir, &repo.repo, &config.github_token)
            .await
        {
            Ok(_) => info!(repo = %repo.repo, "Repo ready"),
            Err(e) => error!(repo = %repo.repo, error = %e, "Failed to clone repo"),
        }
    }

    // Sync repo profiles — onboard any repos that don't have profiles yet
    match onboarding::sync_repo_profiles(&config, &db).await {
        Ok(()) => {}
        Err(e) => warn!(error = %e, "Failed to sync repo profiles"),
    }

    // Spawn the Axum API server (health checks, webhooks)
    let api_config = config.clone();
    let api_db = db.clone();
    let api_handle = tokio::spawn(api::serve(api_config, api_db, api_shutdown_rx));

    // Signal handler — runs in background, sets shutdown flag on SIGTERM/SIGINT.
    // The poll loop checks the flag each iteration and drains gracefully.
    let sig_shutdown = shutdown.clone();
    let sig_notify = shutdown_notify.clone();
    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler");
            tokio::select! {
                _ = ctrl_c => info!("Received SIGINT"),
                _ = sigterm.recv() => info!("Received SIGTERM"),
            }
        }
        #[cfg(not(unix))]
        {
            ctrl_c.await.ok();
            info!("Received SIGINT");
        }
        sig_shutdown.store(true, Ordering::SeqCst);
        sig_notify.notify_waiters(); // Wake poll loop immediately if sleeping
        let _ = api_shutdown_tx.send(()); // Stop API server
        info!("Shutdown signal received, draining current task...");
    });

    // Track config file mtime for hot-reload
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "/etc/karna/config.yaml".to_string());
    let mut last_config_mtime: Option<SystemTime> = std::fs::metadata(&config_path)
        .ok()
        .and_then(|m| m.modified().ok());

    // Main poll loop with graceful shutdown + config hot-reload
    let poll_interval = Duration::from_secs(config.poll_interval_secs);
    let poll_shutdown = shutdown.clone();
    let poll_notify = shutdown_notify.clone();
    let mut needs_rebuild = false;

    let poll_handle = tokio::spawn(async move {
        info!("Poller started (interval: {}s)", config.poll_interval_secs);

        loop {
            // Check shutdown flag before claiming new work
            if poll_shutdown.load(Ordering::SeqCst) {
                info!("Shutdown flag set, exiting poll loop");
                break;
            }

            // Hot-reload config if file changed
            if let Some(current_mtime) = std::fs::metadata(&config_path)
                .ok()
                .and_then(|m| m.modified().ok())
            {
                if last_config_mtime.is_none_or(|last| current_mtime > last) {
                    match config::Config::from_env() {
                        Ok(new_config) => {
                            info!("Config reloaded (file changed on disk)");
                            config = new_config;
                            last_config_mtime = Some(current_mtime);
                        }
                        Err(e) => warn!(error = %e, "Config reload failed, keeping previous"),
                    }
                }
            }

            // Poll for tasks
            match agent::poll_once(&config, &db, &redis).await {
                Ok(()) => {}
                Err(e) => error!("Poll error: {e:#}"),
            }

            // Onboard any repos added via UI (pending or stale profiles)
            match onboarding::check_pending_onboards(&config, &db).await {
                Ok(()) => {}
                Err(e) => error!("Onboarding error: {e:#}"),
            }

            // Check and run due schedules
            match scheduler::check_schedules(&config, &db, &redis).await {
                Ok(()) => {}
                Err(e) => error!("Schedule error: {e:#}"),
            }

            // Check for self-repo updates (after task work is done)
            if let Some(self_repo) = config.self_repo() {
                let repo_path = config.repos_dir.join(self_repo.name());
                match updater::check_self_repo(&repo_path, &self_repo.branch).await {
                    Ok(Some(change)) => {
                        info!(change_type = ?change, "Self-repo update detected");
                        if change.needs_rebuild() {
                            info!("Code changes detected — initiating graceful shutdown for rebuild");
                            needs_rebuild = true;
                            poll_shutdown.store(true, Ordering::SeqCst);
                        } else {
                            info!("Config-only changes — will be picked up via hot-reload");
                        }
                    }
                    Ok(None) => {} // No changes
                    Err(e) => warn!(error = %e, "Self-repo update check failed"),
                }
            }

            // Sleep between polls — but wake instantly on shutdown signal
            tokio::select! {
                _ = tokio::time::sleep(poll_interval) => {}
                _ = poll_notify.notified() => {}
            }
        }

        needs_rebuild
    });

    // Wait for poll loop or API server to exit.
    // Signal handling is async (sets flag → poll loop sees it → breaks → handle completes).
    tokio::select! {
        r = api_handle => error!("API server exited unexpectedly: {r:?}"),
        r = poll_handle => {
            match r {
                Ok(true) => {
                    info!("Exiting with code {EXIT_CODE_UPDATE} for rebuild");
                    std::process::exit(EXIT_CODE_UPDATE);
                }
                Ok(false) => info!("Poller exited cleanly"),
                Err(e) => error!("Poller exited: {e:?}"),
            }
        }
    }

    info!("Agent shutdown complete");
    Ok(())
}
