use anyhow::Result;
use std::path::Path;
use tokio::sync::mpsc;

/// Events streamed from the CLI process as it works.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Agent is invoking a tool (Read, Grep, Bash, etc.)
    ToolUse { tool: String, input_summary: String },
    /// Agent produced text output
    AssistantText(String),
    /// Error during streaming
    Error(String),
}

pub type EventSender = mpsc::UnboundedSender<StreamEvent>;

/// Unified result from any CLI backend (Claude Code, Codex, etc.).
pub struct CliResult {
    pub output: String,
    pub session_id: Option<String>,
    pub cost_usd: f64,
    pub exit_code: i32,
}

/// Unified options passed to any CLI backend.
/// Each backend uses what it supports and ignores the rest.
pub struct CliOptions<'a> {
    pub working_dir: &'a Path,
    pub prompt: &'a str,
    pub system_prompt: Option<&'a str>,
    pub allowed_tools: Option<&'a str>,
    pub max_turns: u32,
    pub model: &'a str,
    pub mcp_config_json: Option<String>,
    pub session_id: Option<&'a str>,
    pub event_tx: Option<EventSender>,
}

/// Summarize tool input for log display.
pub fn summarize_tool_input(tool: &str, input: &serde_json::Value) -> String {
    match tool {
        "Read" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_default(),
        "Grep" => {
            let pattern = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str()).map(shorten_path);
            match path {
                Some(p) => format!("\"{pattern}\" in {p}"),
                None => format!("\"{pattern}\""),
            }
        }
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Bash" => {
            let cmd = input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if cmd.len() > 120 {
                format!("{}…", &cmd[..120])
            } else {
                cmd.to_string()
            }
        }
        "Write" | "Edit" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(shorten_path)
            .unwrap_or_default(),
        "Agent" => input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("subagent")
            .to_string(),
        _ => String::new(),
    }
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

/// Dispatch to the configured CLI backend.
pub async fn run(backend: &str, opts: CliOptions<'_>) -> Result<CliResult> {
    match backend {
        "codex" => crate::codex::run(opts).await,
        "claude" | _ => crate::claude::run(opts).await,
    }
}
