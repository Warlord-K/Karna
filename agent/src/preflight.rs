//! Startup preflight for the CLI backends declared in `agent.backends`.
//!
//! Karna shells out to local agent CLIs that authenticate via the host's
//! subscription login (`~/.cursor`, `~/.grok`, `~/.claude`, `~/.codex`, ...).
//! When the agent runs locally these are inherited from the user's shell — no
//! API keys in `.env` for the subscription backends. This check surfaces a
//! missing binary or a stale login at startup with an actionable hint, instead
//! of failing opaquely halfway through a task.
//!
//! Everything here is best-effort and never fatal: a failed probe only logs a
//! warning. If you containerize Karna later, mount the relevant login dirs
//! (e.g. `~/.cursor`, `~/.grok`) into the agent container so these CLIs stay
//! authenticated.

use std::time::Duration;

use tokio::process::Command;
use tracing::{info, warn};

use crate::config::Config;

/// How a single backend authenticates / is verified at startup.
struct BackendCheck {
    /// Executable name on PATH.
    bin: &'static str,
    /// Args for a fast probe.
    args: &'static [&'static str],
    /// When true, a zero exit code means "authenticated"; when false it only
    /// means "installed".
    is_login_probe: bool,
    /// Actionable hint shown when the probe fails.
    hint: &'static str,
}

fn check_for(cli: &str) -> Option<BackendCheck> {
    Some(match cli {
        "claude" => BackendCheck {
            bin: "claude",
            args: &["--version"],
            is_login_probe: false,
            hint: "install @anthropic-ai/claude-code and run `claude login` (or set CLAUDE_CODE_OAUTH_TOKEN)",
        },
        "codex" => BackendCheck {
            bin: "codex",
            args: &["--version"],
            is_login_probe: false,
            hint: "install @openai/codex and run `codex login`",
        },
        "opencode" => BackendCheck {
            bin: "opencode",
            args: &["--version"],
            is_login_probe: false,
            hint: "install opencode and set OPENROUTER_API_KEY",
        },
        "cursor" => BackendCheck {
            bin: "cursor-agent",
            args: &["status"],
            is_login_probe: true,
            hint: "run `cursor-agent login` on the agent host",
        },
        "grok" => BackendCheck {
            bin: "grok",
            args: &["--version"],
            is_login_probe: false,
            hint: "install grok and run `grok login` on the agent host",
        },
        _ => return None,
    })
}

enum ProbeResult {
    Ok,
    NotFound,
    Failed,
    Timeout,
}

/// Probe every configured backend and log readiness. Never fails.
pub async fn check_backends(config: &Config) {
    for cli in config.backends.keys() {
        let Some(check) = check_for(cli) else {
            info!(backend = %cli, "No preflight check for backend — skipping");
            continue;
        };
        match probe(&check).await {
            ProbeResult::Ok => {
                if check.is_login_probe {
                    info!(backend = %cli, bin = check.bin, "Backend authenticated");
                } else {
                    info!(backend = %cli, bin = check.bin, "Backend CLI available");
                }
            }
            ProbeResult::NotFound => warn!(
                backend = %cli,
                bin = check.bin,
                hint = check.hint,
                "Backend CLI not found on PATH — tasks using this backend will fail"
            ),
            ProbeResult::Failed => {
                if check.is_login_probe {
                    warn!(
                        backend = %cli,
                        bin = check.bin,
                        hint = check.hint,
                        "Backend not authenticated — tasks using this backend will fail"
                    );
                } else {
                    warn!(
                        backend = %cli,
                        bin = check.bin,
                        hint = check.hint,
                        "Backend CLI check failed"
                    );
                }
            }
            ProbeResult::Timeout => warn!(
                backend = %cli,
                bin = check.bin,
                "Backend preflight timed out — assuming available"
            ),
        }
    }
}

async fn probe(check: &BackendCheck) -> ProbeResult {
    let mut cmd = Command::new(check.bin);
    cmd.args(check.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    match tokio::time::timeout(Duration::from_secs(10), cmd.status()).await {
        Err(_) => ProbeResult::Timeout,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ProbeResult::NotFound,
        Ok(Err(_)) => ProbeResult::Failed,
        Ok(Ok(status)) if status.success() => ProbeResult::Ok,
        Ok(Ok(_)) => ProbeResult::Failed,
    }
}
