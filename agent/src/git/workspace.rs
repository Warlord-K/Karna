use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::info;

/// Configure git credential storage so fetch/push/clone all authenticate.
/// Uses the GH_TOKEN env var which `gh` and `git credential-manager` respect,
/// plus a store-based credential helper seeded with the token.
pub async fn configure_git_auth(github_token: &str) -> Result<()> {
    // Write a credential file git can read
    let cred_path = PathBuf::from("/tmp/.git-credentials");
    tokio::fs::write(
        &cred_path,
        format!("https://x-access-token:{github_token}@github.com\n"),
    )
    .await?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let git_dir = Path::new(&home);

    // Tell git to use the credential store
    let _ = Command::new("git")
        .args(["config", "--global", "credential.helper", &format!("store --file={}", cred_path.display())])
        .current_dir(git_dir)
        .output()
        .await;

    Ok(())
}

/// Configure git to sign commits with an SSH key.
/// Called unconditionally at startup — silently skips if no signing config is resolved.
/// Sets gpg.format=ssh, user.signingkey, commit.gpgsign=true,
/// and optionally gpg.ssh.allowedSignersFile for verification.
pub async fn configure_git_signing(signing: Option<&crate::config::SigningConfig>) -> Result<()> {
    let signing = match signing {
        Some(s) => s,
        None => return Ok(()), // No signing configured — silent no-op
    };

    let key_path = &signing.ssh_key_path;

    if !key_path.exists() {
        tracing::warn!(
            path = %key_path.display(),
            "SSH signing key not found, skipping commit signing"
        );
        return Ok(());
    }

    // Copy key to a writable location so we can fix permissions (mount may be :ro)
    let local_key = PathBuf::from("/home/agent/.ssh/signing_key_active");
    tokio::fs::copy(key_path, &local_key).await.with_context(|| {
        format!("Failed to copy signing key from {}", key_path.display())
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&local_key, std::fs::Permissions::from_mode(0o600)).await?;
    }

    let key_str = local_key.to_string_lossy();

    // Set git config for SSH signing
    for (key, value) in [
        ("gpg.format", "ssh"),
        ("user.signingkey", &*key_str),
        ("commit.gpgsign", "true"),
        ("tag.gpgsign", "true"),
    ] {
        let output = Command::new("git")
            .args(["config", "--global", key, value])
            .output()
            .await
            .with_context(|| format!("Failed to set git config {key}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git config --global {key} {value} failed: {stderr}");
        }
    }

    // Optional: set allowed_signers file for verification
    if let Some(signers_path) = &signing.allowed_signers_path {
        if signers_path.exists() {
            let _ = Command::new("git")
                .args([
                    "config", "--global",
                    "gpg.ssh.allowedSignersFile",
                    &signers_path.to_string_lossy(),
                ])
                .output()
                .await;
        }
    }

    info!(key = %key_path.display(), "SSH commit signing configured");
    Ok(())
}

/// Ensure a repo is cloned in the repos directory. Returns the clone path.
pub async fn ensure_cloned(repos_dir: &Path, repo_url: &str, github_token: &str) -> Result<PathBuf> {
    let repo_name = repo_url
        .rsplit('/')
        .next()
        .unwrap_or(repo_url)
        .trim_end_matches(".git");

    let clone_path = repos_dir.join(repo_name);

    if clone_path.exists() {
        info!(repo = repo_name, "Repo already cloned, fetching");
        run_git(&clone_path, &["fetch", "origin"]).await?;
        ensure_agents_md_symlink(&clone_path).await;
    } else {
        // Auth handled globally via credential store (see configure_git_auth)
        let auth_url = if repo_url.starts_with("http") {
            repo_url.to_string()
        } else {
            format!("https://github.com/{repo_url}.git")
        };

        info!(repo = repo_name, "Cloning repo");
        tokio::fs::create_dir_all(repos_dir).await?;
        run_git_in(repos_dir, &["clone", &auth_url, repo_name]).await?;
    }

    ensure_agents_md_symlink(&clone_path).await;

    Ok(clone_path)
}

/// Reset a repo to the latest state of a branch.
pub async fn checkout_and_pull(repo_path: &Path, branch: &str) -> Result<()> {
    run_git(repo_path, &["checkout", branch]).await?;
    run_git(repo_path, &["pull", "origin", branch]).await?;
    Ok(())
}

/// Create a git worktree for isolated task work.
pub async fn create_worktree(
    repo_path: &Path,
    worktree_path: &Path,
    branch_name: &str,
    base_branch: &str,
) -> Result<()> {
    // Clean up existing worktree at this path if it exists
    if worktree_path.exists() {
        let _ = run_git(repo_path, &["worktree", "remove", "--force", &worktree_path.to_string_lossy()]).await;
        let _ = tokio::fs::remove_dir_all(worktree_path).await;
    }

    // Delete the branch if it already exists (from a previous attempt)
    let _ = run_git(repo_path, &["branch", "-D", branch_name]).await;

    // Create worktree with new branch from base
    run_git(
        repo_path,
        &[
            "worktree",
            "add",
            "-b",
            branch_name,
            &worktree_path.to_string_lossy(),
            &format!("origin/{base_branch}"),
        ],
    )
    .await
    .context("Failed to create git worktree")?;

    // Ensure AGENTS.md symlink exists in worktree (for Codex compatibility)
    ensure_agents_md_symlink(worktree_path).await;

    info!(
        worktree = %worktree_path.display(),
        branch = branch_name,
        "Created worktree"
    );

    Ok(())
}

/// Remove a git worktree.
pub async fn remove_worktree(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    let _ = run_git(
        repo_path,
        &["worktree", "remove", "--force", &worktree_path.to_string_lossy()],
    )
    .await;
    Ok(())
}

/// Commit all changes in the worktree.
pub async fn commit_all(worktree_path: &Path, message: &str) -> Result<bool> {
    run_git(worktree_path, &["add", "-A"]).await?;

    // Check if there are changes to commit
    let status = run_git_output(worktree_path, &["status", "--porcelain"]).await?;
    if status.trim().is_empty() {
        return Ok(false);
    }

    run_git(worktree_path, &["commit", "-m", message]).await?;
    Ok(true)
}

/// Push the current branch to origin.
pub async fn push(worktree_path: &Path, branch: &str) -> Result<()> {
    run_git(worktree_path, &["push", "-u", "origin", branch]).await?;
    Ok(())
}

/// Check if the current branch has any commits ahead of its merge-base with the given base branch.
pub async fn has_commits_ahead(worktree_path: &Path, base_branch: &str) -> Result<bool> {
    let base_ref = format!("origin/{base_branch}");
    let output = run_git_output(
        worktree_path,
        &["rev-list", "--count", &format!("{base_ref}..HEAD")],
    )
    .await?;
    let count: i64 = output.trim().parse().unwrap_or(0);
    Ok(count > 0)
}

/// Discover skills and MCP config from a repo/worktree directory.
/// Looks for:
///   - skills/*.md files
///   - .mcp.json or mcp.json at the root
pub async fn discover_repo_extensions(repo_path: &Path) -> RepoExtensions {
    let mut extensions = RepoExtensions::default();

    // Discover skills
    let skills_dir = repo_path.join("skills");
    if skills_dir.is_dir() {
        if let Ok(mut entries) = tokio::fs::read_dir(&skills_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    extensions.skill_paths.push(path);
                }
            }
        }
    }

    // Discover MCP config
    for name in &[".mcp.json", "mcp.json"] {
        let mcp_path = repo_path.join(name);
        if mcp_path.is_file() {
            extensions.mcp_config_path = Some(mcp_path);
            break;
        }
    }

    extensions
}

#[derive(Default, Debug)]
pub struct RepoExtensions {
    /// Paths to skill .md files found in the repo
    pub skill_paths: Vec<PathBuf>,
    /// Path to the repo's MCP config file (.mcp.json or mcp.json)
    pub mcp_config_path: Option<PathBuf>,
}

/// Ensure AGENTS.md exists as a symlink to CLAUDE.md (for Codex compatibility).
/// Codex reads AGENTS.md for project instructions; CLAUDE.md is the canonical source.
/// No-op if CLAUDE.md doesn't exist or AGENTS.md already exists.
pub async fn ensure_agents_md_symlink(repo_path: &Path) {
    let claude_md = repo_path.join("CLAUDE.md");
    let agents_md = repo_path.join("AGENTS.md");

    if claude_md.is_file() && !agents_md.exists() {
        // Relative symlink so it works inside worktrees too
        if let Err(e) = tokio::fs::symlink("CLAUDE.md", &agents_md).await {
            tracing::debug!(
                path = %repo_path.display(),
                error = %e,
                "Failed to create AGENTS.md symlink"
            );
        }
    }
}

/// Get the commit log (one-line format) between a base branch and HEAD.
/// Returns a list of commit subject lines, newest first.
pub async fn commit_log_oneline(worktree_path: &Path, base_branch: &str) -> Result<Vec<String>> {
    let base_ref = format!("origin/{base_branch}");
    let output = run_git_output(
        worktree_path,
        &["log", "--oneline", "--format=%s", &format!("{base_ref}..HEAD")],
    )
    .await?;
    Ok(output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect())
}

// --- Helpers ---

async fn run_git(dir: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to run git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr);
    }
    Ok(())
}

async fn run_git_in(dir: &Path, args: &[&str]) -> Result<()> {
    run_git(dir, args).await
}

async fn run_git_output(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
