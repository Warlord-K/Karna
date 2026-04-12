use anyhow::Result;
use std::path::Path;
use tokio::process::Command;
use tracing::{debug, info};

/// What kind of change was detected in the self-repo.
#[derive(Debug, Clone)]
pub struct SelfRepoChange {
    pub agent_code: bool,
    pub frontend_code: bool,
    pub infrastructure: bool,
    pub config_only: bool,
    pub changed_files: Vec<String>,
    pub local_sha: String,
    pub remote_sha: String,
}

impl SelfRepoChange {
    /// Whether this change requires rebuilding container(s).
    pub fn needs_rebuild(&self) -> bool {
        self.agent_code || self.frontend_code || self.infrastructure
    }

    /// Which docker compose services need rebuilding.
    pub fn services_to_rebuild(&self) -> Vec<&'static str> {
        let mut services = Vec::new();
        if self.agent_code || self.infrastructure {
            services.push("agent");
        }
        if self.frontend_code || self.infrastructure {
            services.push("frontend");
        }
        services
    }
}

/// Check if the self-repo has updates on its remote branch.
/// Returns `Some(change)` if there are new commits, `None` if up-to-date.
///
/// This does NOT pull changes — it only fetches and inspects.
/// The host-side wrapper script handles the actual pull + rebuild.
pub async fn check_self_repo(repo_path: &Path, branch: &str) -> Result<Option<SelfRepoChange>> {
    if !repo_path.exists() {
        return Ok(None);
    }

    // Fetch latest from remote
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["fetch", "origin", branch])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug!(error = %stderr, "Self-repo fetch failed");
        return Ok(None);
    }

    // Compare local HEAD with remote
    let local_sha = git_rev_parse(repo_path, "HEAD").await?;
    let remote_ref = format!("origin/{branch}");
    let remote_sha = git_rev_parse(repo_path, &remote_ref).await?;

    if local_sha == remote_sha {
        return Ok(None);
    }

    info!(
        local = &local_sha[..8],
        remote = &remote_sha[..8],
        "Self-repo has new commits"
    );

    // Get list of changed files
    let diff_output = Command::new("git")
        .current_dir(repo_path)
        .args(["diff", "--name-only", &local_sha, &remote_sha])
        .output()
        .await?;

    let changed_files: Vec<String> = String::from_utf8_lossy(&diff_output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    // Classify changes
    let mut agent_code = false;
    let mut frontend_code = false;
    let mut infrastructure = false;
    let mut config_only = true;

    for file in &changed_files {
        match classify_file(file) {
            FileCategory::AgentCode => {
                agent_code = true;
                config_only = false;
            }
            FileCategory::FrontendCode => {
                frontend_code = true;
                config_only = false;
            }
            FileCategory::Infrastructure => {
                infrastructure = true;
                config_only = false;
            }
            FileCategory::Config => {} // config_only stays true
        }
    }

    Ok(Some(SelfRepoChange {
        agent_code,
        frontend_code,
        infrastructure,
        config_only,
        changed_files,
        local_sha,
        remote_sha,
    }))
}

/// Signal to the host-side updater that a rebuild is needed.
/// Writes change details to a well-known Redis key that the wrapper script can poll.
pub async fn signal_rebuild(
    redis: &redis::Client,
    change: &SelfRepoChange,
) -> Result<()> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let services = change.services_to_rebuild().join(",");
    let value = serde_json::json!({
        "services": services,
        "from": &change.local_sha,
        "to": &change.remote_sha,
        "files": change.changed_files.len(),
    });
    redis::cmd("SET")
        .arg("karna:self_update")
        .arg(value.to_string())
        .arg("EX")
        .arg(3600_u64) // 1 hour TTL
        .query_async::<Option<String>>(&mut conn)
        .await?;

    info!(
        services = services,
        from = &change.local_sha[..8],
        to = &change.remote_sha[..8],
        "Signaled rebuild via Redis"
    );
    Ok(())
}

#[derive(Debug)]
enum FileCategory {
    AgentCode,
    FrontendCode,
    Infrastructure,
    Config,
}

fn classify_file(path: &str) -> FileCategory {
    if path.starts_with("agent/src/")
        || path.starts_with("agent/Cargo")
        || path == "agent/Dockerfile"
    {
        FileCategory::AgentCode
    } else if path.starts_with("frontend/")
        || path == "frontend/Dockerfile"
    {
        FileCategory::FrontendCode
    } else if path == "docker-compose.yml"
        || path.starts_with("migrations/")
    {
        FileCategory::Infrastructure
    } else {
        // skills/, *.md, config*.yaml, instructions, etc.
        FileCategory::Config
    }
}

async fn git_rev_parse(repo_path: &Path, rev: &str) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", rev])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git rev-parse {rev} failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
