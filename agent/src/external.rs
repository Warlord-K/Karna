//! Post PR backlinks onto the originating Linear / ClickUp tasks so the
//! external system stays in sync. All calls are best-effort — missing tokens
//! or transient API errors log a warning and never fail the agent's work.

use anyhow::Result;
use tracing::{info, warn};

use crate::models::AgentTask;

const LINEAR_GRAPHQL_URL: &str = "https://api.linear.app/graphql";

/// Called by the implementer right after a PR is opened. If the task came from
/// an external source we mirror the PR URL back as a comment over there.
pub async fn notify_pr_opened(task: &AgentTask, pr_url: &str) {
    let Some(source) = task.external_source.as_deref() else { return };
    let Some(external_id) = task.external_id.as_deref() else { return };

    let result = match source {
        "linear" => post_linear_comment(external_id, pr_url).await,
        "clickup" => post_clickup_comment(external_id, pr_url).await,
        other => {
            warn!(source = other, "Unknown external_source, skipping PR backlink");
            return;
        }
    };

    match result {
        Ok(true) => info!(task_id = %task.id, source, "PR backlink posted to external task"),
        Ok(false) => info!(task_id = %task.id, source, "No API token configured, skipping backlink"),
        Err(e) => warn!(task_id = %task.id, source, error = %e, "Failed to post PR backlink"),
    }
}

async fn post_linear_comment(issue_id: &str, pr_url: &str) -> Result<bool> {
    let Ok(api_key) = std::env::var("LINEAR_API_KEY") else {
        return Ok(false);
    };

    let mutation = r#"mutation CommentCreate($issueId: String!, $body: String!) {
        commentCreate(input: { issueId: $issueId, body: $body }) {
            success
        }
    }"#;

    let body = serde_json::json!({
        "query": mutation,
        "variables": {
            "issueId": issue_id,
            "body": format!("Karna opened a PR for this issue: {}", pr_url),
        },
    });

    let resp = reqwest::Client::new()
        .post(LINEAR_GRAPHQL_URL)
        .header("Authorization", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Linear API returned {status}: {text}");
    }
    Ok(true)
}

async fn post_clickup_comment(task_id: &str, pr_url: &str) -> Result<bool> {
    let Ok(api_token) = std::env::var("CLICKUP_API_TOKEN") else {
        return Ok(false);
    };

    let url = format!("https://api.clickup.com/api/v2/task/{task_id}/comment");
    let body = serde_json::json!({
        "comment_text": format!("Karna opened a PR for this task: {}", pr_url),
        "notify_all": false,
    });

    let resp = reqwest::Client::new()
        .post(url)
        .header("Authorization", api_token)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("ClickUp API returned {status}: {text}");
    }
    Ok(true)
}
