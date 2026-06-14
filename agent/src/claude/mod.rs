use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info};

use crate::cli::{summarize_tool_input, CliOptions, CliResult, StreamEvent};

/// Default system prompt for the autonomous agent.
const AGENT_SYSTEM_PROMPT: &str = "\
You are Karna, an autonomous coding agent. You work independently without \
human interaction during execution. Never use AskUserQuestion — there is no human \
in the loop. If you are uncertain, make the best judgment call and document your \
reasoning in a code comment or commit message.\n\n\
Follow existing code patterns and conventions in each repository. \
Read CLAUDE.md if it exists for project-specific instructions.\n\n\
Git commit rules:\n\
- Use Conventional Commits: type(scope): description\n\
- Types: feat, fix, refactor, test, chore, perf, ci\n\
- NEVER add Co-Authored-By trailers to commits\n\
- NEVER add Signed-off-by trailers to commits";

/// Run Claude Code CLI in non-interactive (headless) mode with streaming output.
///
/// Uses `--output-format stream-json` to get newline-delimited JSON events,
/// parsed in real-time so callers can observe tool usage and progress.
pub async fn run(opts: CliOptions<'_>) -> Result<CliResult> {
    let mcp_config_path = if let Some(mcp_json) = opts.mcp_config_json.as_deref() {
        Some(write_mcp_config_temp_file(mcp_json).await?)
    } else {
        None
    };

    let result = run_inner(&opts, mcp_config_path.as_deref()).await;

    // Best-effort cleanup so MCP env secrets don't linger in /tmp.
    if let Some(path) = mcp_config_path {
        let _ = tokio::fs::remove_file(path).await;
    }

    result
}

async fn write_mcp_config_temp_file(mcp_json: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("karna-claude-mcp-{}.json", uuid::Uuid::new_v4()));

    tokio::fs::write(&path, mcp_json.as_bytes()).await.with_context(|| {
        format!(
            "Failed to write Claude MCP config temp file: {}",
            path.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .await
            .with_context(|| {
                format!(
                    "Failed to set Claude MCP config temp file permissions: {}",
                    path.display()
                )
            })?;
    }

    Ok(path)
}

async fn run_inner(opts: &CliOptions<'_>, mcp_config_path: Option<&Path>) -> Result<CliResult> {
    let mut cmd = Command::new("claude");
    cmd.current_dir(opts.working_dir);

    // Core flags for headless operation
    cmd.arg("-p");
    cmd.arg("--dangerously-skip-permissions");
    cmd.arg("--verbose");
    cmd.arg("--output-format").arg("stream-json");
    cmd.arg("--max-turns").arg(opts.max_turns.to_string());
    cmd.arg("--model").arg(opts.model);
    cmd.arg("--effort").arg("high");

    // Prevent agent from trying to ask questions
    cmd.arg("--disallowed-tools").arg("AskUserQuestion");

    // Session tracking
    if let Some(sid) = opts.session_id {
        if opts.resume {
            cmd.arg("--resume").arg(sid);
        } else {
            cmd.arg("--session-id").arg(sid);
        }
    }

    // System prompt (separate from task prompt)
    let system_prompt = opts
        .system_prompt
        .map(|s| format!("{AGENT_SYSTEM_PROMPT}\n\n{s}"))
        .unwrap_or_else(|| AGENT_SYSTEM_PROMPT.to_string());
    cmd.arg("--system-prompt").arg(&system_prompt);

    if let Some(tools) = opts.allowed_tools {
        cmd.arg("--allowedTools").arg(tools);
    }

    // MCP config — Claude accepts a path, which avoids secrets in argv.
    if let Some(path) = mcp_config_path {
        cmd.arg("--mcp-config").arg(path);
    }

    // Attach images for vision input
    for image_path in &opts.image_paths {
        cmd.arg("--image").arg(image_path);
    }
    if !opts.image_paths.is_empty() {
        info!(
            image_count = opts.image_paths.len(),
            "Attaching images to Claude Code"
        );
    }

    // Pipe prompt via stdin (avoids Linux MAX_ARG_STRLEN 128KB limit)
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    info!(
        dir = %opts.working_dir.display(),
        model = opts.model,
        max_turns = opts.max_turns,
        "Invoking Claude Code"
    );

    let mut child = cmd
        .spawn()
        .context("Failed to spawn claude CLI — is it installed? Run: npm install -g @anthropic-ai/claude-code")?;

    // Write prompt to stdin then close so the process can proceed
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(opts.prompt.as_bytes()).await?;
        drop(stdin);
    }

    // Read stderr in a background task to avoid pipe deadlocks
    let stderr_handle = {
        let stderr = child.stderr.take().expect("stderr piped");
        tokio::spawn(async move {
            let mut buf = String::new();
            let mut reader = BufReader::new(stderr);
            reader.read_to_string(&mut buf).await.ok();
            buf
        })
    };

    // Stream stdout line-by-line (each line is a JSON event)
    let stdout = child.stdout.take().expect("stdout piped");
    let mut lines = BufReader::new(stdout).lines();

    let mut result_text = String::new();
    let mut session_id = None;
    let mut cost_usd = 0.0;
    let mut is_error_response = false;

    while let Some(line) = lines.next_line().await? {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        match json.get("type").and_then(|v| v.as_str()) {
            Some("assistant") => {
                // Extract tool_use blocks from assistant message content
                if let Some(tx) = &opts.event_tx {
                    if let Some(content) =
                        json.pointer("/message/content").and_then(|v| v.as_array())
                    {
                        for block in content {
                            if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                                let tool = block
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                let input = block
                                    .get("input")
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                let summary = summarize_tool_input(tool, &input);
                                let _ = tx.send(StreamEvent::ToolUse {
                                    tool: tool.to_string(),
                                    input_summary: summary,
                                });
                            }
                        }
                    }
                }
            }
            Some("result") => {
                result_text = json
                    .get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                session_id = json
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                cost_usd = json
                    .get("total_cost_usd")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                is_error_response =
                    json.get("subtype").and_then(|v| v.as_str()) == Some("error_response");
            }
            _ => {}
        }
    }

    let status = child.wait().await?;
    let exit_code = status.code().unwrap_or(-1);
    let stderr = stderr_handle.await.unwrap_or_default();

    debug!(exit_code, stderr_len = stderr.len(), "Claude Code finished");

    if !stderr.is_empty() {
        debug!(stderr = %stderr, "Claude stderr");
    }

    if is_error_response {
        if let Some(tx) = &opts.event_tx {
            let _ = tx.send(StreamEvent::Error(result_text.clone()));
        }
        anyhow::bail!("Claude Code returned error: {result_text}");
    }

    if exit_code != 0 && result_text.is_empty() {
        anyhow::bail!("Claude Code exited with code {exit_code}: {stderr}");
    }

    info!(
        exit_code,
        cost_usd,
        session_id = session_id.as_deref().unwrap_or("none"),
        "Claude Code completed"
    );

    Ok(CliResult {
        output: result_text,
        session_id,
        cost_usd,
        exit_code,
    })
}

#[cfg(test)]
mod tests {
    use super::write_mcp_config_temp_file;

    #[tokio::test]
    async fn writes_mcp_temp_file_with_expected_contents() {
        let mcp_json = r#"{"mcpServers":{"example":{"command":"echo","env":{"TOKEN":"test"}}}}"#;
        let path = write_mcp_config_temp_file(mcp_json)
            .await
            .expect("temp mcp config should be written");

        let content = tokio::fs::read_to_string(&path)
            .await
            .expect("temp mcp config should be readable");
        assert_eq!(content, mcp_json);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = tokio::fs::metadata(&path)
                .await
                .expect("temp mcp config metadata should be readable")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        tokio::fs::remove_file(path)
            .await
            .expect("temp mcp config should be removable");
    }
}
