use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::cli::{summarize_tool_input, CliOptions, CliResult, StreamEvent};

/// Default system prompt prepended to every Cursor Agent invocation.
///
/// `cursor-agent` has no dedicated `--system-prompt` flag, so the agent
/// identity and git rules are folded into the prompt sent over stdin.
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

/// Run the Cursor Agent CLI in headless mode with streaming JSON output.
///
/// `cursor-agent` emits the same `stream-json` event shape as Claude Code
/// (`system`/`user`/`assistant`/`result` envelopes with content blocks), so
/// parsing mirrors `claude::run`. Differences:
///   - no `--system-prompt` flag → preamble is prepended to the stdin prompt
///   - `--force` to allow all tools (the container is the sandbox)
///   - `--resume <id>` for session continuation
///   - auth is a subscription login (`cursor-agent login`), not an API key env
pub async fn run(opts: CliOptions<'_>) -> Result<CliResult> {
    let mut cmd = Command::new("cursor-agent");
    cmd.current_dir(opts.working_dir);

    // Headless: print mode + structured streaming events
    cmd.arg("-p");
    cmd.arg("--output-format").arg("stream-json");

    // Allow every tool without prompting — the Docker container is the sandbox
    // (same rationale as Claude's --dangerously-skip-permissions).
    cmd.arg("--force");
    cmd.arg("--trust");

    cmd.arg("--model").arg(opts.model);

    // Session continuation. cursor-agent resumes a chat by id.
    if let Some(sid) = opts.session_id {
        if opts.resume {
            cmd.arg("--resume").arg(sid);
        }
    }

    // cursor-agent has no image-input flag in headless mode.
    if !opts.image_paths.is_empty() {
        warn!(
            count = opts.image_paths.len(),
            "Cursor backend does not support image inputs — images will be ignored"
        );
    }

    // MCP servers are read from the machine-level cursor config
    // (~/.cursor/mcp.json + project .cursor/mcp.json), not per-task config.
    if opts.mcp_config_json.is_some() {
        debug!(
            "cursor backend reads MCP servers from machine config, not per-task mcp_config_json"
        );
    }

    // Build the prompt sent over stdin. When resuming, only send the new turn
    // (prior context lives in the resumed session).
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

    // Pipe the prompt via stdin (cursor-agent reads stdin when `-p` is given
    // with no positional prompt) to avoid the Linux 128KB per-arg limit.
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    info!(
        dir = %opts.working_dir.display(),
        model = opts.model,
        resuming = opts.resume && opts.session_id.is_some(),
        "Invoking Cursor Agent"
    );

    let mut child = cmd.spawn().context(
        "Failed to spawn cursor-agent CLI — is it installed and logged in? Run: cursor-agent login",
    )?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(full_prompt.as_bytes()).await?;
        drop(stdin);
    }

    // Drain stderr in the background to avoid pipe deadlocks.
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

    let mut result_text = String::new();
    let mut last_assistant_text = String::new();
    let mut session_id: Option<String> = None;
    let mut is_error_response = false;

    while let Some(line) = lines.next_line().await? {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        // Capture the session id from the init envelope (or any event carrying it).
        if session_id.is_none() {
            if let Some(sid) = json.get("session_id").and_then(|v| v.as_str()) {
                session_id = Some(sid.to_string());
            }
        }

        match json.get("type").and_then(|v| v.as_str()) {
            Some("assistant") => {
                if let Some(content) = json.pointer("/message/content").and_then(|v| v.as_array()) {
                    for block in content {
                        match block.get("type").and_then(|v| v.as_str()) {
                            Some("tool_use") => {
                                if let Some(tx) = &opts.event_tx {
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
                            Some("text") => {
                                let text = block.get("text").and_then(|v| v.as_str()).unwrap_or("");
                                if !text.trim().is_empty() {
                                    last_assistant_text = text.to_string();
                                    if let Some(tx) = &opts.event_tx {
                                        let truncated: String =
                                            text.trim().chars().take(300).collect();
                                        let _ = tx.send(StreamEvent::AssistantText(truncated));
                                    }
                                }
                            }
                            _ => {}
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
                if let Some(sid) = json.get("session_id").and_then(|v| v.as_str()) {
                    session_id = Some(sid.to_string());
                }
                is_error_response = json
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
            }
            _ => {}
        }
    }

    let status = child.wait().await?;
    let exit_code = status.code().unwrap_or(-1);
    let stderr = stderr_handle.await.unwrap_or_default();

    debug!(
        exit_code,
        stderr_len = stderr.len(),
        "Cursor Agent finished"
    );
    if !stderr.is_empty() {
        debug!(stderr = %stderr, "Cursor stderr");
    }

    let output = if !result_text.is_empty() {
        result_text
    } else {
        last_assistant_text
    };

    if is_error_response {
        if let Some(tx) = &opts.event_tx {
            let _ = tx.send(StreamEvent::Error(output.clone()));
        }
        anyhow::bail!("Cursor Agent returned error: {output}");
    }

    if exit_code != 0 && output.is_empty() {
        anyhow::bail!("Cursor Agent exited with code {exit_code}: {stderr}");
    }

    info!(
        exit_code,
        session_id = session_id.as_deref().unwrap_or("none"),
        "Cursor Agent completed"
    );

    Ok(CliResult {
        output,
        session_id,
        cost_usd: 0.0,
        exit_code,
    })
}
