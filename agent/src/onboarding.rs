use anyhow::Result;
use tracing::{info, warn, error};

use crate::cli::{self, CliOptions};
use crate::config::Config;
use crate::db::Database;
use crate::git::{github, workspace};
use crate::models::RepoProfile;

/// Sync config repos against DB profiles. Creates pending profiles for any new repos.
/// Onboards repos that are in pending status.
pub async fn sync_repo_profiles(config: &Config, db: &Database) -> Result<()> {
    let user_id = match db.first_user_id().await? {
        Some(id) => id,
        None => {
            // No users yet — skip, will run on next reload
            return Ok(());
        }
    };

    // Ensure all config repos have a profile row
    for repo_config in &config.repos {
        let existing = db.get_repo_profile(&repo_config.repo).await?;
        if existing.is_none() {
            info!(repo = %repo_config.repo, "Creating repo profile for config repo");
            let profile = db.upsert_repo_profile(user_id, &repo_config.repo, &repo_config.branch).await?;
            db.update_repo_sync_issues(profile.id, repo_config.sync_issues).await?;
        } else if let Some(profile) = existing {
            db.update_repo_sync_issues(profile.id, repo_config.sync_issues).await?;
        }
    }

    // Onboard all pending profiles (config repos + any added via UI before startup)
    let profiles = db.get_all_repo_profiles().await?;
    for profile in profiles {
        if profile.status == "pending" {
            info!(repo = %profile.repo, "Onboarding repo");
            match onboard_repo(config, db, &profile).await {
                Ok(()) => info!(repo = %profile.repo, "Repo onboarded successfully"),
                Err(e) => {
                    error!(repo = %profile.repo, error = %e, "Failed to onboard repo");
                    let _ = db.set_repo_profile_error(profile.id, &format!("{e:#}")).await;
                }
            }
        }
    }

    Ok(())
}

/// Onboard a single repo: clone, explore with CLI, parse output, store profile.
async fn onboard_repo(config: &Config, db: &Database, profile: &RepoProfile) -> Result<()> {
    db.set_repo_profile_status(profile.id, "onboarding").await?;

    // Ensure repo is cloned
    let repo_path = workspace::ensure_cloned(&config.repos_dir, &profile.repo, &config.github_token).await?;
    workspace::checkout_and_pull(&repo_path, &profile.branch).await?;

    // Get current commit SHA
    let sha = get_head_sha(&repo_path).await.unwrap_or_default();

    // Build onboarding prompt
    let mut prompt = include_str!("../templates/onboard_prompt.txt").to_string();
    prompt = prompt.replace("{repo}", &profile.repo);
    prompt = prompt.replace("{branch}", &profile.branch);

    // Use cheapest model for onboarding
    let cli_name = config.default_cli();
    let model = "opus";

    info!(repo = %profile.repo, cli = cli_name, model = model, "Invoking CLI for repo onboarding");

    let result = cli::run(cli_name, CliOptions {
        working_dir: &repo_path,
        prompt: &prompt,
        system_prompt: None,
        allowed_tools: Some("Read,Glob,Grep,Bash"),
        max_turns: 30,
        model,
        mcp_config_json: None,
        session_id: None,
        resume: false,
        event_tx: None,
        image_paths: Vec::new(),
    })
    .await?;

    // Parse structured profile from output
    let profile_json = parse_profile_json(&result.output);
    let summary = extract_summary(&result.output);

    db.set_repo_profile_data(
        profile.id,
        &summary,
        profile_json,
        &sha,
        result.cost_usd,
    )
    .await?;

    // Auto-register GitHub webhook if a public URL is configured
    if let Some(webhook_url) = &config.webhook_url {
        match github::ensure_repo_webhook(
            &profile.repo,
            webhook_url,
            config.github_webhook_secret.as_deref(),
        )
        .await
        {
            Ok(()) => info!(repo = %profile.repo, "Webhook configured"),
            Err(e) => warn!(repo = %profile.repo, error = %e,
                "Failed to register webhook (missing admin:repo_hook scope?)"),
        }
    }

    Ok(())
}

/// Check for any pending or stale profiles and onboard them.
/// Called from the poll loop to pick up repos added via UI after startup.
pub async fn check_pending_onboards(config: &Config, db: &Database) -> Result<()> {
    let profiles = db.get_all_repo_profiles().await?;
    let pending: Vec<&RepoProfile> = profiles
        .iter()
        .filter(|p| p.status == "pending" || p.status == "stale")
        .collect();

    if pending.is_empty() {
        return Ok(());
    }

    for profile in pending {
        info!(repo = %profile.repo, status = %profile.status, "Onboarding repo from poll loop");
        match onboard_repo(config, db, profile).await {
            Ok(()) => info!(repo = %profile.repo, "Repo onboarded successfully"),
            Err(e) => {
                error!(repo = %profile.repo, error = %e, "Failed to onboard repo");
                let _ = db.set_repo_profile_error(profile.id, &format!("{e:#}")).await;
            }
        }
    }

    Ok(())
}

#[allow(dead_code)]
pub async fn check_stale_profiles(config: &Config, db: &Database) -> Result<()> {
    let profiles = db.get_ready_repo_profiles().await?;

    for profile in profiles {
        let repo_path = config.repos_dir.join(
            profile.repo.rsplit('/').next().unwrap_or(&profile.repo),
        );
        if !repo_path.exists() {
            continue;
        }

        if let Ok(current_sha) = get_head_sha(&repo_path).await {
            if let Some(stored_sha) = &profile.last_commit_sha {
                if *stored_sha != current_sha {
                    info!(repo = %profile.repo, "Repo profile is stale (HEAD changed)");
                    db.set_repo_profile_status(profile.id, "stale").await?;
                }
            }
        }
    }

    Ok(())
}

/// Format all ready repo profiles into a string suitable for injection into planning prompts.
pub fn format_profiles_for_prompt(profiles: &[RepoProfile]) -> String {
    if profiles.is_empty() {
        return String::new();
    }

    let mut out = String::from("## Repository Profiles\n\nThe following summaries describe each configured repository. Use these to determine which repos need changes for this task. You do NOT need to explore repos whose profiles indicate they are irrelevant.\n\n");

    for profile in profiles {
        out.push_str(&format!("### {}\n", profile.repo));

        // Add structured info line if available
        if let Some(json) = &profile.profile_json {
            let lang = json.get("language").and_then(|v| v.as_str()).unwrap_or("unknown");
            let framework = json.get("framework").and_then(|v| v.as_str());
            let mut info_parts = vec![format!("Language: {lang}")];
            if let Some(fw) = framework {
                if fw != "null" {
                    info_parts.push(format!("Framework: {fw}"));
                }
            }
            info_parts.push(format!("Branch: {}", profile.branch));
            out.push_str(&format!("{}\n\n", info_parts.join(" | ")));
        }

        if let Some(summary) = &profile.summary {
            out.push_str(summary);
            out.push_str("\n\n");
        }
    }

    out
}

/// Parse the structured JSON profile from CLI output.
fn parse_profile_json(output: &str) -> serde_json::Value {
    let json_str = output
        .find("<!-- profile")
        .and_then(|start| {
            let after = &output[start..];
            after.find("profile -->").map(|end| &after[12..end])
        });

    match json_str {
        Some(s) => {
            let trimmed = s.trim();
            match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(e) => {
                    warn!(error = %e, "Failed to parse profile JSON from onboarding output");
                    serde_json::json!({})
                }
            }
        }
        None => {
            warn!("No <!-- profile ... profile --> block found in onboarding output");
            serde_json::json!({})
        }
    }
}

/// Extract the summary section from CLI output (everything before the profile block).
fn extract_summary(output: &str) -> String {
    // Try to find the ## Summary header
    if let Some(start) = output.find("## Summary") {
        let rest = &output[start..];
        // Take until the profile block or end
        if let Some(end) = rest.find("<!-- profile") {
            return rest[..end].trim().to_string();
        }
        return rest.trim().to_string();
    }

    // Fallback: take everything before the profile block
    if let Some(end) = output.find("<!-- profile") {
        return output[..end].trim().to_string();
    }

    output.trim().to_string()
}

/// Get the HEAD commit SHA of a repo.
async fn get_head_sha(repo_path: &std::path::Path) -> Result<String> {
    let output = tokio::process::Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
