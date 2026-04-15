use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::cli::{CliOptions, CliResult, StreamEvent};

/// Default system prompt prepended to every Codex invocation.
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

/// Run OpenAI Codex CLI in non-interactive mode with JSONL streaming.
///
/// Uses `codex exec` subcommand with:
///   --full-auto       — skip confirmations (workspace-write sandbox)
///   --json            — JSONL event stream on stdout
///   --model MODEL     — model selection (o4-mini, o3, gpt-5.4, etc.)
///   prompt via stdin  — piped (exec subcommand supports it)
pub async fn run(opts: CliOptions<'_>) -> Result<CliResult> {
    let mut cmd = Command::new("codex");
    cmd.current_dir(opts.working_dir);

    // Resume existing session or start a new exec
    let resuming = opts.session_id.is_some();
    if let Some(sid) = opts.session_id {
        cmd.arg("exec").arg("resume").arg(sid);
    } else {
        cmd.arg("exec");
    }

    // Skip sandbox + approvals — the Docker container is the sandbox
    // (same rationale as Claude Code's --dangerously-skip-permissions)
    cmd.arg("--dangerously-bypass-approvals-and-sandbox");

    // JSONL streaming output (structured events like Claude's stream-json)
    cmd.arg("--json");

    // Model
    cmd.arg("--model").arg(opts.model);

    // Warn if images are attached — Codex doesn't support image input
    if !opts.image_paths.is_empty() {
        warn!(
            count = opts.image_paths.len(),
            "Codex backend does not support image inputs — images will be ignored"
        );
    }

    // Build the full prompt: preamble + system prompt + task prompt
    // When resuming, only send the new prompt (context is in the session)
    let full_prompt = if resuming {
        let mut p = opts.prompt.to_string();
        if !opts.image_paths.is_empty() {
            let filenames: Vec<String> = opts.image_paths.iter()
                .filter_map(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
                .collect();
            p.push_str(&format!(
                "\n\nNote: This task has {} attached image(s) that cannot be displayed in this backend. The image filenames are: {}. Ask the user for text descriptions if needed.",
                opts.image_paths.len(), filenames.join(", ")
            ));
        }
        p
    } else {
        let mut parts = vec![AGENT_PREAMBLE.to_string()];
        if let Some(sys) = opts.system_prompt {
            parts.push(sys.to_string());
        }
        let mut task_prompt = opts.prompt.to_string();
        if !opts.image_paths.is_empty() {
            let filenames: Vec<String> = opts.image_paths.iter()
                .filter_map(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
                .collect();
            task_prompt.push_str(&format!(
                "\n\nNote: This task has {} attached image(s) that cannot be displayed in this backend. The image filenames are: {}. Ask the user for text descriptions if needed.",
                opts.image_paths.len(), filenames.join(", ")
            ));
        }
        parts.push(task_prompt);
        parts.join("\n\n")
    };

    // Pipe prompt via stdin (codex exec reads from stdin when piped)
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    info!(
        dir = %opts.working_dir.display(),
        model = opts.model,
        resuming,
        "Invoking Codex CLI"
    );

    let mut child = cmd
        .spawn()
        .context("Failed to spawn codex CLI — is it installed? Run: npm install -g @openai/codex")?;

    // Write prompt to stdin then close
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(full_prompt.as_bytes()).await?;
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

    // Stream JSONL events from stdout
    let stdout = child.stdout.take().expect("stdout piped");
    let mut lines = BufReader::new(stdout).lines();

    let mut last_agent_message = String::new();
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut session_id: Option<String> = None;

    while let Some(line) = lines.next_line().await? {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Capture session_id from any event that carries it
        if session_id.is_none() {
            if let Some(sid) = json.get("session_id").and_then(|v| v.as_str()) {
                session_id = Some(sid.to_string());
            }
        }

        match event_type {
            // Item lifecycle events — extract tool usage and messages
            "item.started" | "item.completed" => {
                if let Some(item) = json.get("item") {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");

                    if let Some(tx) = &opts.event_tx {
                        match item_type {
                            "command_execution" => {
                                let command = item
                                    .get("command")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let summary = if command.len() > 120 {
                                    format!("{}…", &command[..120])
                                } else {
                                    command.to_string()
                                };
                                let _ = tx.send(StreamEvent::ToolUse {
                                    tool: "Bash".to_string(),
                                    input_summary: summary,
                                });
                            }
                            "file_change" if event_type == "item.completed" => {
                                if let Some(changes) =
                                    item.get("changes").and_then(|v| v.as_array())
                                {
                                    for change in changes {
                                        let path = change
                                            .get("path")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown");
                                        let kind = change
                                            .get("kind")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("update");
                                        let tool = match kind {
                                            "add" => "Write",
                                            "delete" => "Delete",
                                            _ => "Edit",
                                        };
                                        let _ = tx.send(StreamEvent::ToolUse {
                                            tool: tool.to_string(),
                                            input_summary: shorten_path(path),
                                        });
                                    }
                                }
                            }
                            "mcp_tool_call" if event_type == "item.started" => {
                                let server = item
                                    .get("server")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("mcp");
                                let tool = item
                                    .get("tool")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                let _ = tx.send(StreamEvent::ToolUse {
                                    tool: format!("{server}/{tool}"),
                                    input_summary: String::new(),
                                });
                            }
                            "agent_message" if event_type == "item.completed" => {
                                let text = item
                                    .get("text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                if !text.trim().is_empty() {
                                    last_agent_message = text.to_string();
                                    let truncated: String = text.trim().chars().take(300).collect();
                                    let _ =
                                        tx.send(StreamEvent::AssistantText(truncated));
                                }
                            }
                            _ => {}
                        }
                    }

                    // Track last agent message regardless of event_tx
                    if item_type == "agent_message" && event_type == "item.completed" {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            if !text.trim().is_empty() {
                                last_agent_message = text.to_string();
                            }
                        }
                    }
                }
            }
            // Token usage from turn completion
            "turn.completed" => {
                if let Some(usage) = json.get("usage") {
                    total_input_tokens +=
                        usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    total_output_tokens +=
                        usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                }
            }
            "turn.failed" => {
                let error_msg = json
                    .pointer("/error/message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                if let Some(tx) = &opts.event_tx {
                    let _ = tx.send(StreamEvent::Error(error_msg.to_string()));
                }
            }
            _ => {}
        }
    }

    let status = child.wait().await?;
    let exit_code = status.code().unwrap_or(-1);
    let stderr = stderr_handle.await.unwrap_or_default();

    debug!(exit_code, stderr_len = stderr.len(), "Codex CLI finished");

    if !stderr.is_empty() {
        debug!(stderr = %stderr, "Codex stderr");
    }

    if exit_code != 0 && last_agent_message.is_empty() {
        anyhow::bail!("Codex CLI exited with code {exit_code}: {stderr}");
    }

    info!(
        exit_code,
        input_tokens = total_input_tokens,
        output_tokens = total_output_tokens,
        "Codex CLI completed"
    );

    Ok(CliResult {
        output: last_agent_message,
        session_id,
        cost_usd: 0.0,
        exit_code,
    })
}

/// Show last 3 path components to keep logs readable.
fn shorten_path(path: &str) -> String {
    let parts: Vec<&str> = path.rsplit('/').take(3).collect();
    if parts.len() < 3 {
        return path.to_string();
    }
    let shortened: Vec<&str> = parts.into_iter().rev().collect();
    format!("…/{}", shortened.join("/"))
}
