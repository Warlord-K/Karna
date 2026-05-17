use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::info;

pub struct PrInfo {
    pub url: String,
    pub number: i32,
}

/// Create a pull request using `gh` CLI.
pub async fn create_pr(
    worktree_path: &Path,
    repo: &str,
    branch: &str,
    base_branch: &str,
    title: &str,
    body: &str,
) -> Result<PrInfo> {
    info!(repo, branch, "Creating PR");

    let output = Command::new("gh")
        .current_dir(worktree_path)
        .args([
            "pr", "create",
            "--repo", repo,
            "--head", branch,
            "--base", base_branch,
            "--title", title,
            "--body", body,
        ])
        .output()
        .await
        .context("Failed to run gh pr create")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If PR already exists, get its URL
        if stderr.contains("already exists") {
            return get_existing_pr(worktree_path, repo, branch).await;
        }
        anyhow::bail!("gh pr create failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let pr_url = stdout.lines().last().unwrap_or(&stdout).trim().to_string();

    let number = extract_pr_number(&pr_url).unwrap_or(0);

    info!(pr_url = %pr_url, number, "PR created");
    Ok(PrInfo { url: pr_url, number })
}

/// Get an existing PR for a branch.
async fn get_existing_pr(worktree_path: &Path, repo: &str, branch: &str) -> Result<PrInfo> {
    let output = Command::new("gh")
        .current_dir(worktree_path)
        .args([
            "pr", "view", branch,
            "--repo", repo,
            "--json", "url,number",
        ])
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)?;

    Ok(PrInfo {
        url: json["url"].as_str().unwrap_or("").to_string(),
        number: json["number"].as_i64().unwrap_or(0) as i32,
    })
}

/// Get review comments from a PR.
pub async fn get_pr_comments(
    worktree_path: &Path,
    repo: &str,
    pr_number: i32,
) -> Result<Vec<String>> {
    let output = Command::new("gh")
        .current_dir(worktree_path)
        .args([
            "pr", "view",
            &pr_number.to_string(),
            "--repo", repo,
            "--json", "reviews,comments",
        ])
        .output()
        .await?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_default();

    let mut comments = Vec::new();

    if let Some(reviews) = json["reviews"].as_array() {
        for review in reviews {
            if let Some(body) = review["body"].as_str() {
                if !body.is_empty() {
                    comments.push(body.to_string());
                }
            }
        }
    }

    if let Some(issue_comments) = json["comments"].as_array() {
        for comment in issue_comments {
            if let Some(body) = comment["body"].as_str() {
                if !body.is_empty() {
                    comments.push(body.to_string());
                }
            }
        }
    }

    Ok(comments)
}

#[allow(dead_code)]
pub async fn check_pr_status(
    worktree_path: &Path,
    repo: &str,
    pr_number: i32,
) -> Result<String> {
    let output = Command::new("gh")
        .current_dir(worktree_path)
        .args([
            "pr", "checks",
            &pr_number.to_string(),
            "--repo", repo,
        ])
        .output()
        .await?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn extract_pr_number(url: &str) -> Option<i32> {
    url.rsplit('/').next()?.parse().ok()
}

/// Outcome of `ensure_repo_webhook` — distinguishes "we registered it" from
/// "GitHub already had one." `matched_url` is the URL GitHub actually has
/// registered, which may differ cosmetically from `webhook_url` (trailing
/// slash, case in scheme/host).
#[derive(Debug)]
pub struct WebhookEnsureResult {
    pub matched_url: String,
    pub created: bool,
}

/// Compare two URLs in a forgiving way: case-insensitive scheme + host,
/// trailing-slash insensitive on the path. Anything else must match exactly.
fn webhook_urls_equivalent(a: &str, b: &str) -> bool {
    fn normalize(u: &str) -> String {
        let trimmed = u.trim_end_matches('/');
        // Split scheme://host[:port]/path — fold the prefix lowercase.
        if let Some(idx) = trimmed.find("://") {
            let (scheme, rest) = trimmed.split_at(idx + 3);
            if let Some(slash) = rest.find('/') {
                let (host, path) = rest.split_at(slash);
                format!("{}{}{}", scheme.to_lowercase(), host.to_lowercase(), path)
            } else {
                format!("{}{}", scheme.to_lowercase(), rest.to_lowercase())
            }
        } else {
            trimmed.to_lowercase()
        }
    }
    normalize(a) == normalize(b)
}

/// Ensure a GitHub webhook is registered on the repo for agent PR events.
/// Idempotent — checks existing hooks first and skips if one already points
/// to the same URL. Requires the PAT to have `admin:repo_hook` scope.
pub async fn ensure_repo_webhook(
    repo: &str,
    webhook_url: &str,
    secret: Option<&str>,
) -> Result<WebhookEnsureResult> {
    let hook_target = format!("{webhook_url}/webhooks/github");

    // Check if a webhook already exists for this URL
    let list_output = Command::new("gh")
        .args(["api", &format!("repos/{repo}/hooks"), "--jq", ".[].config.url"])
        .output()
        .await
        .context("Failed to list repo webhooks")?;

    if list_output.status.success() {
        let existing = String::from_utf8_lossy(&list_output.stdout);
        for line in existing.lines() {
            let url = line.trim();
            if url.is_empty() {
                continue;
            }
            if webhook_urls_equivalent(url, &hook_target) {
                info!(repo, url, "Webhook already registered");
                return Ok(WebhookEnsureResult {
                    matched_url: url.to_string(),
                    created: false,
                });
            }
        }
    }

    // Create webhook via gh api with field flags
    let mut args = vec![
        "api".to_string(),
        format!("repos/{repo}/hooks"),
        "--method".to_string(), "POST".to_string(),
        "-f".to_string(), format!("config[url]={hook_target}"),
        "-f".to_string(), "config[content_type]=json".to_string(),
        "-f".to_string(), "config[insecure_ssl]=0".to_string(),
        "-F".to_string(), "active=true".to_string(),
        "-f".to_string(), "events[]=pull_request_review".to_string(),
        "-f".to_string(), "events[]=issue_comment".to_string(),
        "-f".to_string(), "events[]=pull_request".to_string(),
        "-f".to_string(), "events[]=issues".to_string(),
    ];
    if let Some(s) = secret {
        args.push("-f".to_string());
        args.push(format!("config[secret]={s}"));
    }

    let result = Command::new("gh")
        .args(&args)
        .output()
        .await
        .context("Failed to create webhook via gh api")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        if stderr.contains("Not Found") || stderr.contains("404") {
            anyhow::bail!(
                "Cannot create webhook on {repo}: 404. \
                 Ensure GITHUB_TOKEN has admin:repo_hook scope."
            );
        }
        anyhow::bail!("Failed to create webhook on {repo}: {stderr}");
    }

    info!(repo, url = %hook_target, "Webhook registered");
    Ok(WebhookEnsureResult { matched_url: hook_target, created: true })
}
