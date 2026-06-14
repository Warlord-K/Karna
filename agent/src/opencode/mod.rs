use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::cli::{CliOptions, CliResult, StreamEvent};

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

/// Run opencode CLI in non-interactive mode (`opencode run`).
///
/// Model is passed through verbatim as `provider/model` (e.g.
/// `openrouter/moonshotai/kimi-k2.6`). Auth is handled by env (typically
/// `OPENROUTER_API_KEY`), inherited from the agent process.
///
/// `--format json` produces a stream of JSON event lines on stdout; the schema
/// is best-effort parsed for tool calls and assistant text. If parsing fails,
/// the run still completes — the raw final message is returned as `output`.
pub async fn run(opts: CliOptions<'_>) -> Result<CliResult> {
    let mut cmd = Command::new("opencode");
    cmd.arg("run");

    // The Docker container is the sandbox — same posture as the other backends.
    cmd.arg("--dangerously-skip-permissions");
    cmd.arg("--format").arg("json");
    cmd.arg("--dir").arg(opts.working_dir);
    cmd.arg("-m").arg(opts.model);

    // Session continuation. opencode prefers `-s <id>`; `--continue` is a separate
    // "last session" shortcut we don't need here.
    if let Some(sid) = opts.session_id {
        if opts.resume {
            cmd.arg("-s").arg(sid);
        }
    }

    if !opts.image_paths.is_empty() {
        warn!(
            count = opts.image_paths.len(),
            "opencode backend image support is best-effort — attaching via --file"
        );
        for image_path in &opts.image_paths {
            cmd.arg("--file").arg(image_path);
        }
    }

    // opencode has no separate system prompt flag — prepend instead.
    // When resuming we only send the new turn (context lives in the session).
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

    if opts.mcp_config_json.is_some() {
        // MCP servers are picked up from ~/.config/opencode/opencode.json,
        // written at agent startup by config::write_opencode_global_config().
        // The per-task `mcp_config_json` (Claude format) isn't used here.
        debug!(
            "opencode backend reads MCP servers from opencode.json, not per-task mcp_config_json"
        );
    }

    // opencode takes the prompt as a positional argument.
    cmd.arg(&full_prompt);

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    info!(
        dir = %opts.working_dir.display(),
        model = opts.model,
        "Invoking opencode CLI"
    );

    let mut child = cmd
        .spawn()
        .context("Failed to spawn opencode CLI — is it installed? Run: curl -fsSL https://opencode.ai/install | bash")?;

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

    let mut last_assistant_text = String::new();
    let mut last_result_text = String::new();
    let mut session_id: Option<String> = None;
    let mut cost_usd: f64 = 0.0;
    let mut saw_any_json = false;

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            // opencode in `--format json` can interleave non-JSON banner lines on stderr;
            // anything that lands here unparsed is treated as raw text.
            if last_assistant_text.is_empty() {
                last_assistant_text = trimmed.to_string();
            }
            continue;
        };
        saw_any_json = true;

        if session_id.is_none() {
            if let Some(sid) = json
                .get("session_id")
                .or_else(|| json.get("sessionID"))
                .or_else(|| json.pointer("/session/id"))
                .and_then(|v| v.as_str())
            {
                session_id = Some(sid.to_string());
            }
        }

        if let Some(usd) = json
            .get("cost_usd")
            .or_else(|| json.get("total_cost_usd"))
            .or_else(|| json.pointer("/usage/cost_usd"))
            .and_then(|v| v.as_f64())
        {
            cost_usd = usd.max(cost_usd);
        }

        let event_type = json
            .get("type")
            .or_else(|| json.get("event"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match event_type {
            "tool_use" | "tool_call" | "tool.call" | "tool.use" => {
                if let Some(tx) = &opts.event_tx {
                    let tool = json
                        .get("name")
                        .or_else(|| json.get("tool"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let input = json
                        .get("input")
                        .or_else(|| json.get("arguments"))
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    let summary = crate::cli::summarize_tool_input(tool, &input);
                    let _ = tx.send(StreamEvent::ToolUse {
                        tool: tool.to_string(),
                        input_summary: summary,
                    });
                }
            }
            "assistant" | "message" | "assistant.message" | "text" => {
                let text = json
                    .get("text")
                    .or_else(|| json.get("content"))
                    .or_else(|| json.pointer("/message/text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !text.trim().is_empty() {
                    last_assistant_text = text.to_string();
                    if let Some(tx) = &opts.event_tx {
                        let truncated: String = text.trim().chars().take(300).collect();
                        let _ = tx.send(StreamEvent::AssistantText(truncated));
                    }
                }
            }
            "result" | "done" | "finish" => {
                let text = json
                    .get("result")
                    .or_else(|| json.get("text"))
                    .or_else(|| json.get("output"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !text.is_empty() {
                    last_result_text = text.to_string();
                }
            }
            "error" => {
                let msg = json
                    .get("message")
                    .or_else(|| json.pointer("/error/message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                if let Some(tx) = &opts.event_tx {
                    let _ = tx.send(StreamEvent::Error(msg.to_string()));
                }
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
        saw_any_json,
        "opencode CLI finished"
    );
    if !stderr.is_empty() {
        debug!(stderr = %stderr, "opencode stderr");
    }

    let output = if !last_result_text.is_empty() {
        last_result_text
    } else {
        last_assistant_text
    };

    if exit_code != 0 && output.is_empty() {
        anyhow::bail!("opencode CLI exited with code {exit_code}: {stderr}");
    }

    info!(
        exit_code,
        cost_usd,
        session_id = session_id.as_deref().unwrap_or("none"),
        "opencode CLI completed"
    );

    Ok(CliResult {
        output,
        session_id,
        cost_usd,
        exit_code,
    })
}
