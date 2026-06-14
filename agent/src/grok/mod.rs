use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::cli::{CliOptions, CliResult, StreamEvent};

/// Default system prompt prepended to every Grok invocation.
///
/// Grok's `--system-prompt-override` *replaces* the built-in coding system
/// prompt, which would drop its default tool-use behavior. Instead the agent
/// identity and git rules are folded into the prompt content, matching the
/// codex/opencode backends.
const AGENT_PREAMBLE: &str = "\
You are Karna, an autonomous coding agent. You work independently without \
human interaction during execution. If you are uncertain, make the best judgment \
call and document your reasoning in a code comment or commit message.\n\n\
Follow existing code patterns and conventions in each repository. \
Read AGENTS.md or CLAUDE.md if they exist for project-specific instructions.\n\n\
Git commit rules:\n\
- Use Conventional Commits: type(scope): description\n\
- Types: feat, fix, refactor, test, chore, perf, ci\n\
- NEVER add Co-Authored-By trailers to commits\n\
- NEVER add Signed-off-by trailers to commits";

/// Flush buffered assistant text as a stream event once this many unsent
/// characters accumulate. Grok streams word-level `text` deltas, so emitting
/// each one individually would flood `agent_logs`.
const TEXT_FLUSH_CHARS: usize = 200;

/// Run the Grok CLI (`grok`) in headless single-turn mode with streaming JSON.
///
/// Grok's `streaming-json` output emits `{"type":"thought"|"text","data":...}`
/// deltas followed by `{"type":"end","sessionId":...,"stopReason":...}`. Tool
/// calls are executed internally and not surfaced as discrete stream events, so
/// no `ToolUse` events are produced for this backend (best-effort mapping).
///
/// The prompt is written to a temp file and passed via `--prompt-file` to avoid
/// the Linux 128KB per-arg limit. Auth is a subscription login (`grok login`).
pub async fn run(opts: CliOptions<'_>) -> Result<CliResult> {
    // Build the prompt. When resuming, only send the new turn (prior context
    // lives in the resumed session).
    let full_prompt = if opts.session_id.is_some() && opts.resume {
        opts.prompt.to_string()
    } else {
        let mut parts = vec![AGENT_PREAMBLE.to_string()];
        if let Some(sys) = opts.system_prompt {
            parts.push(sys.to_string());
        }
        parts.push(opts.prompt.to_string());
        parts.join("\n\n")
    };

    // Write the prompt to a temp file so large prompts (skills + diffs) don't
    // hit the per-arg size limit. Cleaned up before returning.
    let prompt_path = std::env::temp_dir().join(format!("karna-grok-{}.txt", uuid::Uuid::new_v4()));
    tokio::fs::write(&prompt_path, full_prompt.as_bytes())
        .await
        .with_context(|| {
            format!(
                "Failed to write grok prompt file: {}",
                prompt_path.display()
            )
        })?;

    let result = run_inner(&opts, &prompt_path).await;

    // Best-effort cleanup of the temp prompt file.
    let _ = tokio::fs::remove_file(&prompt_path).await;

    result
}

async fn run_inner(opts: &CliOptions<'_>, prompt_path: &std::path::Path) -> Result<CliResult> {
    let mut cmd = Command::new("grok");

    // Run inline (no TUI alt-screen) in headless single-turn mode.
    cmd.arg("--no-alt-screen");
    cmd.arg("--cwd").arg(opts.working_dir);
    cmd.arg("--prompt-file").arg(prompt_path);
    cmd.arg("--output-format").arg("streaming-json");

    // The Docker container is the sandbox — bypass per-tool approvals.
    cmd.arg("--permission-mode").arg("bypassPermissions");

    cmd.arg("-m").arg(opts.model);
    cmd.arg("--max-turns").arg(opts.max_turns.to_string());

    // Session continuation.
    if let Some(sid) = opts.session_id {
        if opts.resume {
            cmd.arg("--resume").arg(sid);
        }
    }

    if !opts.image_paths.is_empty() {
        warn!(
            count = opts.image_paths.len(),
            "Grok backend does not support image inputs — images will be ignored"
        );
    }

    // MCP servers are read from grok's machine-level config, not per-task.
    if opts.mcp_config_json.is_some() {
        debug!("grok backend reads MCP servers from machine config, not per-task mcp_config_json");
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    info!(
        dir = %opts.working_dir.display(),
        model = opts.model,
        resuming = opts.resume && opts.session_id.is_some(),
        "Invoking Grok CLI"
    );

    let mut child = cmd
        .spawn()
        .context("Failed to spawn grok CLI — is it installed and logged in? Run: grok login")?;

    let stderr_handle = {
        let stderr = child.stderr.take().expect("stderr piped");
        tokio::spawn(async move {
            let mut buf = String::new();
            let mut reader = BufReader::new(stderr);
            reader.read_to_string(&mut buf).await.ok();
            buf
        })
    };

    let stdout = child.stdout.take().expect("stdout piped");
    let mut lines = BufReader::new(stdout).lines();

    let mut output = String::new();
    let mut unflushed = String::new();
    let mut session_id: Option<String> = None;
    let mut error_message: Option<String> = None;

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };

        match json.get("type").and_then(|v| v.as_str()).unwrap_or("") {
            // Assistant text deltas — accumulate and flush in chunks.
            "text" => {
                if let Some(data) = json.get("data").and_then(|v| v.as_str()) {
                    output.push_str(data);
                    unflushed.push_str(data);
                    if unflushed.chars().count() >= TEXT_FLUSH_CHARS {
                        if let Some(tx) = &opts.event_tx {
                            let _ =
                                tx.send(StreamEvent::AssistantText(std::mem::take(&mut unflushed)));
                        } else {
                            unflushed.clear();
                        }
                    }
                }
            }
            // Reasoning deltas — tracked but not streamed to logs (too noisy).
            "thought" => {}
            "end" => {
                if let Some(sid) = json.get("sessionId").and_then(|v| v.as_str()) {
                    session_id = Some(sid.to_string());
                }
            }
            "error" => {
                let msg = json
                    .get("message")
                    .or_else(|| json.pointer("/error/message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
                    .to_string();
                if let Some(tx) = &opts.event_tx {
                    let _ = tx.send(StreamEvent::Error(msg.clone()));
                }
                error_message = Some(msg);
            }
            _ => {}
        }
    }

    // Flush any trailing assistant text.
    if !unflushed.is_empty() {
        if let Some(tx) = &opts.event_tx {
            let _ = tx.send(StreamEvent::AssistantText(std::mem::take(&mut unflushed)));
        }
    }

    let status = child.wait().await?;
    let exit_code = status.code().unwrap_or(-1);
    let stderr = stderr_handle.await.unwrap_or_default();

    debug!(exit_code, stderr_len = stderr.len(), "Grok CLI finished");
    if !stderr.is_empty() {
        debug!(stderr = %stderr, "Grok stderr");
    }

    let output = output.trim().to_string();

    if let Some(msg) = error_message {
        if output.is_empty() {
            anyhow::bail!("Grok CLI returned error: {msg}");
        }
    }

    if exit_code != 0 && output.is_empty() {
        anyhow::bail!("Grok CLI exited with code {exit_code}: {stderr}");
    }

    info!(
        exit_code,
        session_id = session_id.as_deref().unwrap_or("none"),
        "Grok CLI completed"
    );

    Ok(CliResult {
        output,
        session_id,
        cost_usd: 0.0,
        exit_code,
    })
}
