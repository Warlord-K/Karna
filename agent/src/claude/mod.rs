use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
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
        cmd.arg("--session-id").arg(sid);
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

    // MCP config — can be passed as JSON string directly
    if let Some(mcp_json) = &opts.mcp_config_json {
        cmd.arg("--mcp-config").arg(mcp_json);
    }

    // Attach images for vision input
    for image_path in &opts.image_paths {
        cmd.arg("--image").arg(image_path);
    }
    if !opts.image_paths.is_empty() {
        info!(image_count = opts.image_paths.len(), "Attaching images to Claude Code");
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
        use tokio::io::AsyncWriteExt;
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
