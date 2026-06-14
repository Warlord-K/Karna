//! Policy scan — match a task's plan against active policies.
//!
//! Called from the planner after `set_plan`. Extracts file paths mentioned in
//! the plan_content markdown, matches them against each active policy's
//! `repo_pattern` + `path_glob`, and stores the hits on the task as JSON so
//! the UI can render a banner on the plan_review tab.
//!
//! This is advisory: severity == 'warn' is just a visual nudge; severity ==
//! 'block' will (in a future patch) dim the approve button. The planner does
//! not gate the state transition either way.

use anyhow::Result;
use karna_shared::models::AgentTask;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use tracing::{debug, warn};

use crate::db::Database;

/// Pull every file-path-looking token out of a plan_content markdown body.
///
/// Heuristic: matches things that look like `dir/file.ext` or `dir/sub/file`.
/// Markdown link / inline-code wrappers are stripped first. Cheap and
/// stupid on purpose — we'd rather over-match (extra banner) than miss a real
/// hit (no banner on a migration).
pub fn extract_paths(markdown: &str) -> Vec<String> {
    let cleaned = markdown.replace(['`', '[', ']', '(', ')', '"', '\''], " ");

    let mut out: BTreeSet<String> = BTreeSet::new();
    for token in cleaned.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = token.trim_matches(|c: char| c == '.' || c == ':' || c == '*');
        if t.is_empty() {
            continue;
        }
        // Must contain a slash and have at least one extension or another segment
        if !t.contains('/') {
            continue;
        }
        // Filter obvious noise: URLs, version numbers, line ranges, etc.
        if t.contains("://") || t.starts_with("http") {
            continue;
        }
        // Reject anything with characters that aren't valid in paths.
        if t.chars()
            .any(|c| matches!(c, '<' | '>' | '|' | '?' | '\\' | '{' | '}'))
        {
            continue;
        }
        // Strip a trailing `:line:col` if Claude wrote `path/to/file.ts:42:7`
        let mut s = t.to_string();
        if let Some(idx) = s.find(':') {
            s.truncate(idx);
        }
        if s.is_empty() || !s.contains('/') {
            continue;
        }
        out.insert(s);
    }
    out.into_iter().collect()
}

/// Glob match: supports `**` (any path segments) and `*` (any chars except `/`).
/// Returns true on a full-string match.
pub fn glob_match(pattern: &str, candidate: &str) -> bool {
    glob_match_inner(pattern.as_bytes(), candidate.as_bytes())
}

fn glob_match_inner(pat: &[u8], s: &[u8]) -> bool {
    // Tokenize the pattern into segments separated by `/`. Each segment is
    // either `**` (matches any number of path segments) or a regular glob
    // segment (matches one segment with `*` wildcards).
    let pat_segs: Vec<&[u8]> = split_segments(pat);
    let s_segs: Vec<&[u8]> = split_segments(s);
    glob_segments(&pat_segs, &s_segs)
}

fn split_segments(buf: &[u8]) -> Vec<&[u8]> {
    if buf.is_empty() {
        return Vec::new();
    }
    buf.split(|&b| b == b'/').collect()
}

fn glob_segments(pat: &[&[u8]], s: &[&[u8]]) -> bool {
    if pat.is_empty() {
        return s.is_empty();
    }
    if pat[0] == b"**" {
        // `**` matches zero or more whole segments.
        for skip in 0..=s.len() {
            if glob_segments(&pat[1..], &s[skip..]) {
                return true;
            }
        }
        return false;
    }
    if s.is_empty() {
        return false;
    }
    if !segment_match(pat[0], s[0]) {
        return false;
    }
    glob_segments(&pat[1..], &s[1..])
}

/// Match a single path segment with `*` wildcards (no `/`).
fn segment_match(pat: &[u8], s: &[u8]) -> bool {
    let mut pi = 0;
    let mut si = 0;
    let mut star: Option<(usize, usize)> = None;
    while si < s.len() {
        if pi < pat.len() && pat[pi] == b'*' {
            star = Some((pi, si));
            pi += 1;
        } else if pi < pat.len() && pat[pi] == s[si] {
            pi += 1;
            si += 1;
        } else if let Some((p_star, s_star)) = star {
            pi = p_star + 1;
            si = s_star + 1;
            star = Some((p_star, si));
        } else {
            return false;
        }
    }
    while pi < pat.len() && pat[pi] == b'*' {
        pi += 1;
    }
    pi == pat.len()
}

/// Repo-pattern match. Three cases:
///   "*"           → matches any repo
///   "owner/*"     → matches any repo under that owner
///   "owner/repo"  → exact match
fn repo_match(pattern: &str, repo: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return repo.starts_with(prefix) && repo.as_bytes().get(prefix.len()) == Some(&b'/');
    }
    pattern == repo
}

/// Scan a task's plan + repos against every active policy. Returns the JSON
/// array suitable for `agent_tasks.policy_matches`, or `None` when nothing fires.
pub async fn scan_task(db: &Database, task: &AgentTask) -> Result<Option<Value>> {
    let plan = match task.plan_content.as_deref() {
        Some(p) if !p.is_empty() => p,
        _ => return Ok(None),
    };
    let policies = db.list_active_policies().await?;
    if policies.is_empty() {
        return Ok(None);
    }

    let paths = extract_paths(plan);
    if paths.is_empty() {
        return Ok(None);
    }

    // Task repos: comma-separated, single repo, or all configured repos for
    // multi-repo parents (no `repo` set). We approximate the latter as "*".
    let task_repos: Vec<&str> = task.repos();

    let mut arr: Vec<Value> = Vec::new();
    for policy in &policies {
        let repo_ok = if task_repos.is_empty() {
            // Multi-repo parent: only consider policies that target all repos.
            policy.repo_pattern == "*"
        } else {
            task_repos
                .iter()
                .any(|r| repo_match(&policy.repo_pattern, r))
        };
        if !repo_ok {
            continue;
        }
        let matched: Vec<&str> = paths
            .iter()
            .filter(|p| glob_match(&policy.path_glob, p))
            .map(|s| s.as_str())
            .collect();
        if matched.is_empty() {
            continue;
        }
        arr.push(json!({
            "policy_id": policy.id,
            "name": policy.name,
            "severity": policy.severity,
            "message": policy.message,
            "paths": matched,
        }));
    }

    if arr.is_empty() {
        return Ok(None);
    }
    Ok(Some(Value::Array(arr)))
}

/// Run a scan and persist the result. Logs and swallows errors — policy scan
/// is advisory and must never fail the planner.
pub async fn scan_and_persist(db: &Database, task: &AgentTask) {
    match scan_task(db, task).await {
        Ok(matches) => {
            if let Err(e) = db.set_task_policy_matches(task.id, matches.as_ref()).await {
                warn!(error = %e, task_id = %task.id, "Failed to persist policy matches");
            } else if let Some(arr) = matches {
                debug!(task_id = %task.id, count = arr.as_array().map(|a| a.len()), "Policy matches recorded");
            }
        }
        Err(e) => warn!(error = %e, task_id = %task.id, "Policy scan failed"),
    }
}
