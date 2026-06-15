use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Events streamed from the CLI process as it works.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Agent is invoking a tool (Read, Grep, Bash, etc.)
    ToolUse { tool: String, input_summary: String },
    /// Tool execution completed with output text.
    ToolResult { tool: String, output: String },
    /// Agent produced text output
    AssistantText(String),
    /// Error during streaming
    Error(String),
}

pub type EventSender = mpsc::UnboundedSender<StreamEvent>;

/// Unified result from any CLI backend (Claude Code, Codex, etc.).
#[allow(dead_code)]
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
    pub resume: bool,
    pub event_tx: Option<EventSender>,
    pub image_paths: Vec<PathBuf>,
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
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
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
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
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

pub const TOOL_OUTPUT_MAX_CHARS: usize = 3000;

pub fn truncate_for_log(text: &str, max_chars: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
}

/// Flatten CLI tool result blocks into displayable plain text.
pub fn summarize_tool_output(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(summarize_tool_output)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        serde_json::Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                return text.trim().to_string();
            }
            if let Some(content) = map.get("content") {
                let nested = summarize_tool_output(content);
                if !nested.is_empty() {
                    return nested;
                }
            }
            if let Some(output) = map.get("output") {
                let nested = summarize_tool_output(output);
                if !nested.is_empty() {
                    return nested;
                }
            }
            serde_json::to_string(map).unwrap_or_default()
        }
        other => other.to_string(),
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

#[cfg(test)]
mod tests {
    use super::{summarize_tool_output, truncate_for_log, TOOL_OUTPUT_MAX_CHARS};

    #[test]
    fn truncates_tool_output_to_log_cap() {
        let long = "a".repeat(TOOL_OUTPUT_MAX_CHARS + 64);
        let truncated = truncate_for_log(&long, TOOL_OUTPUT_MAX_CHARS);
        assert!(truncated.ends_with('…'));
        assert_eq!(truncated.chars().count(), TOOL_OUTPUT_MAX_CHARS + 1);
    }

    #[test]
    fn flattens_nested_tool_output_blocks() {
        let value = serde_json::json!([
            {"type": "text", "text": "line one"},
            {"type": "text", "text": "line two"}
        ]);
        assert_eq!(summarize_tool_output(&value), "line one\nline two");
    }
}

/// Dispatch to the configured CLI backend.
pub async fn run(backend: &str, opts: CliOptions<'_>) -> Result<CliResult> {
    match backend {
        "codex" => crate::codex::run(opts).await,
        "opencode" => crate::opencode::run(opts).await,
        "cursor" => crate::cursor::run(opts).await,
        "grok" => crate::grok::run(opts).await,
        _ => crate::claude::run(opts).await,
    }
}
