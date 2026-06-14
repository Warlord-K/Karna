use std::collections::HashSet;

use serde_json::Value;
use tracing::warn;
use uuid::Uuid;

use crate::config::MemoryConfig;
use crate::models::AgentTask;

const SEARCH_PATHS: &[&str] = &["/search", "/v2/memories/search/", "/v1/memories/search/"];
const ADD_PATHS: &[&str] = &["/memories", "/v1/memories/"];

#[derive(Clone)]
pub struct MemoryClient {
    enabled: bool,
    base_url: String,
    http: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryItem {
    pub id: Option<String>,
    pub text: String,
    pub score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySnippet {
    pub namespace: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySection {
    pub text: String,
    pub item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddPayload {
    Text(String),
    Messages(Vec<MemoryMessage>),
}

impl MemoryClient {
    pub fn new(config: &MemoryConfig) -> Self {
        Self {
            enabled: config.enabled && !config.url.trim().is_empty(),
            base_url: normalize_base_url(&config.url),
            http: reqwest::Client::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Search memory. Failure-tolerant by design: any API/parse errors return
    /// an empty result so memory can never block task execution.
    pub async fn search(&self, query: &str, namespace: &str, limit: usize) -> Vec<MemoryItem> {
        if !self.enabled || query.trim().is_empty() || namespace.trim().is_empty() || limit == 0 {
            return Vec::new();
        }

        let payload = serde_json::json!({
            "query": query,
            "user_id": namespace,
            "limit": limit,
            "top_k": limit,
        });

        for path in SEARCH_PATHS {
            let url = format!("{}{}", self.base_url, path);
            let resp = match self.http.post(&url).json(&payload).send().await {
                Ok(resp) => resp,
                Err(err) => {
                    warn!(namespace, path, error = %err, "Memory search request failed");
                    continue;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!(namespace, path, %status, body, "Memory search returned non-success status");
                continue;
            }

            let json = match resp.json::<Value>().await {
                Ok(json) => json,
                Err(err) => {
                    warn!(namespace, path, error = %err, "Memory search response was not valid JSON");
                    continue;
                }
            };

            let mut items = parse_search_response(&json);
            if items.len() > limit {
                items.truncate(limit);
            }
            return items;
        }

        Vec::new()
    }

    /// Best-effort write. All failures are swallowed with warnings.
    pub async fn add(&self, payload: AddPayload, namespace: &str) {
        if !self.enabled || namespace.trim().is_empty() {
            return;
        }

        let messages = match payload {
            AddPayload::Text(text) => {
                let cleaned = normalize_whitespace(&text);
                if cleaned.is_empty() {
                    return;
                }
                vec![serde_json::json!({ "role": "user", "content": cleaned })]
            }
            AddPayload::Messages(messages) => {
                let mut normalized = Vec::new();
                for message in messages {
                    let role = normalize_whitespace(&message.role);
                    let content = normalize_whitespace(&message.content);
                    if role.is_empty() || content.is_empty() {
                        continue;
                    }
                    normalized.push(serde_json::json!({ "role": role, "content": content }));
                }
                if normalized.is_empty() {
                    return;
                }
                normalized
            }
        };

        let body = serde_json::json!({
            "messages": messages,
            "user_id": namespace,
        });

        for path in ADD_PATHS {
            let url = format!("{}{}", self.base_url, path);
            let resp = match self.http.post(&url).json(&body).send().await {
                Ok(resp) => resp,
                Err(err) => {
                    warn!(namespace, path, error = %err, "Memory add request failed");
                    continue;
                }
            };

            if resp.status().is_success() {
                return;
            }

            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(namespace, path, %status, body, "Memory add returned non-success status");
        }
    }
}

pub fn repo_namespace(repo: &str) -> String {
    format!("repo:{}", repo.trim())
}

pub fn agent_namespace(profile_slug: &str) -> String {
    format!("agent:{}", profile_slug.trim())
}

pub fn user_namespace(user_id: Uuid) -> String {
    format!("user:{user_id}")
}

pub fn profile_slug(cli: &str, model: &str) -> String {
    let raw = format!("{cli}-{model}");
    raw.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn dedupe_snippets(snippets: Vec<MemorySnippet>) -> Vec<MemorySnippet> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for snippet in snippets {
        let key = normalize_whitespace(&snippet.text).to_lowercase();
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        deduped.push(snippet);
    }
    deduped
}

pub fn build_memory_section(
    snippets: &[MemorySnippet],
    max_items: usize,
    max_chars: usize,
) -> Option<MemorySection> {
    if snippets.is_empty() || max_items == 0 || max_chars < "## Memory\n".chars().count() {
        return None;
    }

    let mut section = String::from("## Memory\n");
    let mut count = 0usize;

    for snippet in snippets.iter().take(max_items) {
        let text = normalize_whitespace(&snippet.text);
        if text.is_empty() {
            continue;
        }

        let line = format!("- [{}] {}", snippet.namespace.trim(), text);
        let remaining = max_chars.saturating_sub(section.chars().count());
        if remaining == 0 {
            break;
        }

        let rendered = if line.chars().count() > remaining {
            if remaining <= 3 {
                break;
            }
            format!("{}...", truncate_chars(&line, remaining - 3))
        } else {
            line
        };

        section.push_str(&rendered);
        section.push('\n');
        count += 1;

        if section.chars().count() >= max_chars {
            break;
        }
    }

    if count == 0 {
        return None;
    }

    Some(MemorySection {
        text: section.trim_end().to_string(),
        item_count: count,
    })
}

/// Keep summaries concise and cheap for write-back.
pub fn summarize_task_for_memory(task: &AgentTask) -> Option<String> {
    let title = normalize_whitespace(&task.title);
    if title.is_empty() {
        return None;
    }

    let mut parts = vec![format!("Task: {title}")];

    if let Some(repo) = task.repo.as_deref() {
        let repo = normalize_whitespace(repo);
        if !repo.is_empty() {
            parts.push(format!("Repo: {repo}"));
        }
    }

    if let Some(description) = task.description.as_deref() {
        let compact = normalize_whitespace(description);
        if !compact.is_empty() {
            parts.push(format!("Context: {}", truncate_chars(&compact, 300)));
        }
    }

    if let Some(plan) = task.plan_content.as_deref() {
        let compact = normalize_whitespace(plan);
        if !compact.is_empty() {
            parts.push(format!("Plan: {}", truncate_chars(&compact, 800)));
        }
    }

    if let Some(pr_url) = task.pr_url.as_deref() {
        let pr_url = normalize_whitespace(pr_url);
        if !pr_url.is_empty() {
            parts.push(format!("PR: {pr_url}"));
        }
    }

    Some(truncate_chars(&parts.join("\n"), 1_600))
}

fn normalize_base_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

fn parse_search_response(json: &Value) -> Vec<MemoryItem> {
    let candidates = match json {
        Value::Array(items) => Some(items),
        Value::Object(map) => map
            .get("results")
            .and_then(Value::as_array)
            .or_else(|| map.get("memories").and_then(Value::as_array))
            .or_else(|| {
                map.get("data")
                    .and_then(|v| v.get("results"))
                    .and_then(Value::as_array)
            })
            .or_else(|| map.get("data").and_then(Value::as_array)),
        _ => None,
    };

    let Some(candidates) = candidates else {
        return Vec::new();
    };

    let mut items = Vec::new();
    for item in candidates {
        let text = item
            .get("memory")
            .and_then(Value::as_str)
            .or_else(|| item.get("text").and_then(Value::as_str))
            .or_else(|| item.get("content").and_then(Value::as_str))
            .map(normalize_whitespace)
            .unwrap_or_default();

        if text.is_empty() {
            continue;
        }

        let id = item
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let score = item.get("score").and_then(Value::as_f64);

        items.push(MemoryItem { id, text, score });
    }

    items
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaces_match_expected_format() {
        assert_eq!(repo_namespace("owner/repo"), "repo:owner/repo");
        assert_eq!(agent_namespace("claude-sonnet"), "agent:claude-sonnet");
        assert_eq!(
            user_namespace(Uuid::nil()),
            "user:00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn profile_slug_is_stable() {
        assert_eq!(profile_slug("Claude", "Sonnet 4.5"), "claude-sonnet-4-5");
        assert_eq!(profile_slug("codex", "gpt-5.4-mini"), "codex-gpt-5-4-mini");
    }

    #[test]
    fn memory_section_respects_item_and_char_budgets() {
        let snippets = vec![
            MemorySnippet {
                namespace: "repo:owner/repo".to_string(),
                text: "Use uv for Python dependency management.".to_string(),
            },
            MemorySnippet {
                namespace: "agent:codex-gpt-5-4".to_string(),
                text: "CI expects cargo clippy --workspace with no warnings.".to_string(),
            },
        ];

        let section = build_memory_section(&snippets, 1, 200).expect("section");
        assert_eq!(section.item_count, 1);
        assert!(section.text.contains("## Memory"));
        assert!(section.text.contains("repo:owner/repo"));

        let section = build_memory_section(&snippets, 5, 55).expect("section");
        assert!(section.text.chars().count() <= 55);
    }

    #[test]
    fn parse_search_response_handles_common_shapes() {
        let json = serde_json::json!({
            "results": [
                {"id": "m1", "memory": "Use postgres:16.", "score": 0.92},
                {"id": "m2", "text": "Run clippy before opening PR."}
            ]
        });
        let items = parse_search_response(&json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.as_deref(), Some("m1"));
        assert_eq!(items[0].text, "Use postgres:16.");

        let json = serde_json::json!([
            {"content": "Array response works too."},
            {"memory": "   "}
        ]);
        let items = parse_search_response(&json);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "Array response works too.");
    }
}
