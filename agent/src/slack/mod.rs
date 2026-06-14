use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::db::Database;
use crate::models::AgentTask;

const SLACK_OPEN_CONNECTION_URL: &str = "https://slack.com/api/apps.connections.open";
const SLACK_OPEN_DM_URL: &str = "https://slack.com/api/conversations.open";
const SLACK_POST_MESSAGE_URL: &str = "https://slack.com/api/chat.postMessage";

#[derive(Debug, Clone, Copy)]
pub enum NotificationKind {
    PlanReady,
    PrOpened,
    TaskFailed,
    TaskDone,
}

pub fn spawn_socket_mode(config: Config, db: Database) {
    if !config.slack.enabled {
        info!("Slack Socket Mode disabled in config");
        return;
    }
    if config.slack.bot_token.is_none() || config.slack.app_token.is_none() {
        warn!("Slack enabled but SLACK_BOT_TOKEN/SLACK_APP_TOKEN missing; Socket Mode not started");
        return;
    }

    tokio::spawn(async move {
        if let Err(e) = run_socket_mode(config, db).await {
            warn!(error = %e, "Slack Socket Mode exited");
        }
    });
}

pub async fn send_task_notification(
    config: &Config,
    db: &Database,
    task: &AgentTask,
    kind: NotificationKind,
) -> Result<()> {
    if !config.slack.enabled {
        return Ok(());
    }
    let Some(bot_token) = config.slack.bot_token.as_deref() else {
        return Ok(());
    };

    let Some(channel) = resolve_notification_channel(config, task, bot_token).await? else {
        return Ok(());
    };

    let text = format_notification(kind, task);
    let blocks = notification_blocks(kind, task);
    let response = post_message(
        bot_token,
        &channel,
        &text,
        task.slack_thread_ts.as_deref(),
        blocks,
    )
    .await?;

    if task.slack_thread_ts.is_none() {
        db.set_task_slack_thread(task.id, &channel, &response.ts)
            .await
            .context("failed to persist Slack thread mapping")?;
    }

    Ok(())
}

/// Post an arbitrary task message to Slack, reusing the task's mapped thread
/// when available. Returns a lightweight reference in "channel:thread_ts"
/// format when the message is accepted.
pub async fn send_task_message(
    config: &Config,
    db: &Database,
    task: &AgentTask,
    text: &str,
) -> Result<Option<String>> {
    if !config.slack.enabled {
        return Ok(None);
    }
    let Some(bot_token) = config.slack.bot_token.as_deref() else {
        return Ok(None);
    };
    let Some(channel) = resolve_notification_channel(config, task, bot_token).await? else {
        return Ok(None);
    };
    let response = post_message(
        bot_token,
        &channel,
        text,
        task.slack_thread_ts.as_deref(),
        None,
    )
    .await?;
    let thread_ts = task
        .slack_thread_ts
        .clone()
        .unwrap_or_else(|| response.ts.clone());
    if task.slack_thread_ts.is_none() {
        db.set_task_slack_thread(task.id, &channel, &thread_ts)
            .await
            .context("failed to persist Slack thread mapping")?;
    }
    Ok(Some(format!("{channel}:{thread_ts}")))
}

async fn run_socket_mode(config: Config, db: Database) -> Result<()> {
    let app_token = config
        .slack
        .app_token
        .as_deref()
        .context("SLACK_APP_TOKEN missing")?;
    let mut backoff = 1u64;

    loop {
        let session = run_socket_session(&config, &db, app_token).await;
        if let Err(e) = session {
            warn!(error = %e, backoff_secs = backoff, "Slack socket session ended");
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

async fn run_socket_session(config: &Config, db: &Database, app_token: &str) -> Result<()> {
    let ws_url = open_socket_mode_connection(app_token).await?;
    let (mut socket, _) = connect_async(&ws_url)
        .await
        .with_context(|| format!("failed to connect to Slack websocket: {ws_url}"))?;

    info!("Slack Socket Mode connected");

    while let Some(frame) = socket.next().await {
        match frame {
            Ok(Message::Text(payload)) => {
                let envelope: SocketEnvelope = match serde_json::from_str(&payload) {
                    Ok(v) => v,
                    Err(e) => {
                        debug!(error = %e, "Ignoring non-JSON socket payload");
                        continue;
                    }
                };

                if let Some(envelope_id) = envelope.envelope_id.as_deref() {
                    let ack = json!({ "envelope_id": envelope_id }).to_string();
                    if let Err(e) = socket.send(Message::Text(ack.into())).await {
                        warn!(error = %e, "Failed to ACK Slack envelope");
                        break;
                    }
                }

                handle_envelope(config, db, envelope).await;
            }
            Ok(Message::Ping(payload)) => {
                if let Err(e) = socket.send(Message::Pong(payload)).await {
                    warn!(error = %e, "Failed to respond to Slack ping");
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                warn!("Slack socket closed by remote");
                break;
            }
            Ok(_) => {}
            Err(e) => {
                return Err(anyhow::anyhow!("slack websocket read error: {e}"));
            }
        }
    }

    Ok(())
}

async fn handle_envelope(config: &Config, db: &Database, envelope: SocketEnvelope) {
    match envelope.kind.as_str() {
        "events_api" => {
            if let Some(payload) = envelope.payload {
                if let Err(e) = handle_events_api(config, db, payload).await {
                    warn!(error = %e, "Failed to handle Slack events_api envelope");
                }
            }
        }
        "interactive" => {
            if let Some(payload) = envelope.payload {
                if let Err(e) = handle_interactive(config, db, payload).await {
                    warn!(error = %e, "Failed to handle Slack interactive envelope");
                }
            }
        }
        _ => {}
    }
}

async fn handle_events_api(
    config: &Config,
    db: &Database,
    payload: serde_json::Value,
) -> Result<()> {
    let Some(event) = payload.get("event") else {
        return Ok(());
    };
    let Some(event_type) = event.get("type").and_then(|v| v.as_str()) else {
        return Ok(());
    };

    match event_type {
        "app_mention" | "message" => handle_message_event(config, db, event).await?,
        _ => {}
    }
    Ok(())
}

async fn handle_message_event(
    config: &Config,
    db: &Database,
    event: &serde_json::Value,
) -> Result<()> {
    if event.get("subtype").is_some() || event.get("bot_id").is_some() {
        return Ok(());
    }

    let Some(slack_user) = event.get("user").and_then(|v| v.as_str()) else {
        return Ok(());
    };

    let Some(channel) = event.get("channel").and_then(|v| v.as_str()) else {
        return Ok(());
    };
    let text = event
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim();
    if text.is_empty() {
        return Ok(());
    }
    let event_type = event
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let is_explicit_operator_surface = is_operator_surface(event_type, text);
    let is_operator_user = is_allowlisted_user(config, slack_user);

    let ts = event.get("ts").and_then(|v| v.as_str()).unwrap_or_default();
    let thread_ts = event
        .get("thread_ts")
        .and_then(|v| v.as_str())
        .unwrap_or(ts);

    if let Some(task) = db.find_task_by_slack_thread(channel, thread_ts).await? {
        let accepts_external_replies = task
            .orchestrator_config()
            .map(|cfg| cfg.accepts_external_replies)
            .unwrap_or(false);

        if !is_operator_user {
            if !accepts_external_replies {
                return Ok(());
            }
            route_thread_feedback(db, &task, text).await?;
            send_plain_reply(
                config,
                channel,
                thread_ts,
                "Captured. I routed this thread reply to the orchestrator turn.",
            )
            .await?;
            return Ok(());
        }
        if is_explicit_operator_surface {
            if let Some(command) = parse_command(text) {
                let Some(karna_user_id) = resolve_operator_user_id(config, slack_user) else {
                    return Ok(());
                };
                handle_command(
                    config,
                    db,
                    karna_user_id,
                    slack_user,
                    channel,
                    thread_ts,
                    command,
                )
                .await?;
                return Ok(());
            }
        }

        route_thread_feedback(db, &task, text).await?;
        send_plain_reply(
            config,
            channel,
            thread_ts,
            "Captured. I routed this as feedback to the task.",
        )
        .await?;
        return Ok(());
    }

    if !is_explicit_operator_surface {
        // TODO: watched non-task threads will be checked in this branch once
        // provider-verification tracking is added.
        return Ok(());
    }
    if !is_operator_user {
        return Ok(());
    }
    let Some(karna_user_id) = resolve_operator_user_id(config, slack_user) else {
        return Ok(());
    };

    if let Some(command) = parse_command(text) {
        handle_command(
            config,
            db,
            karna_user_id,
            slack_user,
            channel,
            thread_ts,
            command,
        )
        .await?;
    }

    Ok(())
}

async fn handle_interactive(
    config: &Config,
    db: &Database,
    payload: serde_json::Value,
) -> Result<()> {
    let interaction_type = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if interaction_type != "block_actions" {
        return Ok(());
    }

    let slack_user = payload
        .pointer("/user/id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !is_allowlisted_user(config, slack_user) {
        return Ok(());
    }

    let channel = payload
        .pointer("/channel/id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if channel.is_empty() {
        return Ok(());
    }
    let thread_ts = payload
        .pointer("/container/thread_ts")
        .and_then(|v| v.as_str())
        .or_else(|| {
            payload
                .pointer("/message/thread_ts")
                .and_then(|v| v.as_str())
        })
        .or_else(|| payload.pointer("/message/ts").and_then(|v| v.as_str()))
        .unwrap_or_default();
    if thread_ts.is_empty() {
        return Ok(());
    }

    let action_id = payload
        .pointer("/actions/0/action_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let Some(action) = parse_gate_action(action_id) else {
        return Ok(());
    };

    let Some(task) = db.get_task(action.task_id).await? else {
        send_plain_reply(
            config,
            channel,
            thread_ts,
            "Task not found for this action.",
        )
        .await?;
        return Ok(());
    };

    match action.stage {
        GateStage::Plan => match action.decision {
            GateDecision::Approve => {
                if task.status == "plan_review" {
                    db.update_status(task.id, "in_progress").await?;
                }
                db.insert_log(task.id, "slack", "Plan approved from Slack", "info", None)
                    .await?;
                send_plain_reply(
                    config,
                    channel,
                    thread_ts,
                    "Plan approved. Implementation started.",
                )
                .await?;
            }
            GateDecision::RequestChanges => {
                db.insert_log(
                    task.id,
                    "slack",
                    "Plan changes requested from Slack",
                    "comment",
                    None,
                )
                .await?;
                db.set_feedback(task.id, "Slack: requested plan changes")
                    .await?;
                if task.status == "plan_review" {
                    db.update_status(task.id, "planning").await?;
                }
                send_plain_reply(
                    config,
                    channel,
                    thread_ts,
                    "Requested changes sent back to planning.",
                )
                .await?;
            }
            GateDecision::Cancel => {
                db.insert_log(
                    task.id,
                    "slack",
                    "Task cancelled from Slack plan gate",
                    "warning",
                    None,
                )
                .await?;
                db.update_status(task.id, "cancelled").await?;
                send_plain_reply(config, channel, thread_ts, "Task cancelled.").await?;
            }
        },
        GateStage::Review => match action.decision {
            GateDecision::Approve => {
                db.insert_log(
                    task.id,
                    "slack",
                    "Review approved from Slack (awaiting PR merge)",
                    "info",
                    None,
                )
                .await?;
                send_plain_reply(
                    config,
                    channel,
                    thread_ts,
                    "Approved. Merge the PR to mark the task done.",
                )
                .await?;
            }
            GateDecision::RequestChanges => {
                db.insert_log(
                    task.id,
                    "slack",
                    "PR changes requested from Slack",
                    "comment",
                    None,
                )
                .await?;
                db.set_feedback(task.id, "Slack: requested PR changes")
                    .await?;
                if task.status == "review" {
                    db.update_status(task.id, "in_progress").await?;
                }
                send_plain_reply(
                    config,
                    channel,
                    thread_ts,
                    "Requested changes sent to the implementer.",
                )
                .await?;
            }
            GateDecision::Cancel => {
                db.insert_log(
                    task.id,
                    "slack",
                    "Task cancelled from Slack review gate",
                    "warning",
                    None,
                )
                .await?;
                db.update_status(task.id, "cancelled").await?;
                send_plain_reply(config, channel, thread_ts, "Task cancelled.").await?;
            }
        },
    }

    Ok(())
}

async fn handle_command(
    config: &Config,
    db: &Database,
    user_id: Uuid,
    slack_user: &str,
    channel: &str,
    thread_ts: &str,
    command: CommandKind,
) -> Result<()> {
    match command {
        CommandKind::Implement { ticket } => {
            if let Some(existing) = db.find_task_by_external("linear", &ticket).await? {
                let msg = format!(
                    "Already tracking `{ticket}` as task `{}` (`{}`).",
                    existing.id, existing.status
                );
                send_plain_reply(config, channel, thread_ts, &msg).await?;
                return Ok(());
            }

            let external_url = format!("https://linear.app/issue/{ticket}");
            let description = format!(
                "Requested from Slack by <@{slack_user}>.\n\nLinear ticket: {external_url}"
            );
            let title = format!("{ticket}: Implement from Slack");
            let task = db
                .create_task_full(
                    user_id,
                    &title,
                    Some(&description),
                    None,
                    "medium",
                    None,
                    None,
                    None,
                    None,
                    Some("linear"),
                    Some(&ticket),
                    Some(&external_url),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;
            db.set_task_slack_thread(task.id, channel, thread_ts)
                .await?;
            db.insert_log(
                task.id,
                "slack",
                "Task created from Slack implement command",
                "info",
                None,
            )
            .await?;

            let msg = format!(
                "Created task `{}` for `{ticket}`. I'll post updates in this thread.",
                task.id
            );
            send_plain_reply(config, channel, thread_ts, &msg).await?;
        }
        CommandKind::New {
            repo,
            title,
            description,
        } => {
            let task = db
                .create_task_full(
                    user_id,
                    &title,
                    Some(&description),
                    repo.as_deref(),
                    "medium",
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;
            db.set_task_slack_thread(task.id, channel, thread_ts)
                .await?;
            db.insert_log(
                task.id,
                "slack",
                "Task created from Slack new command",
                "info",
                None,
            )
            .await?;

            let msg = format!("Created task `{}` (`{}`).", task.id, task.title);
            send_plain_reply(config, channel, thread_ts, &msg).await?;
        }
        CommandKind::Status | CommandKind::List => {
            let tasks = db.list_tasks_for_user(user_id).await?;
            let non_terminal: Vec<&AgentTask> = tasks
                .iter()
                .filter(|t| !matches!(t.status.as_str(), "done" | "failed" | "cancelled"))
                .take(8)
                .collect();

            let text = if non_terminal.is_empty() {
                "No active tasks right now.".to_string()
            } else {
                let lines = non_terminal
                    .iter()
                    .map(|t| {
                        format!(
                            "• `{}` {} — `{}`",
                            t.task_number.unwrap_or(0),
                            t.title,
                            t.status
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("Active tasks:\n{lines}")
            };
            send_plain_reply(config, channel, thread_ts, &text).await?;
        }
        CommandKind::Cancel { query } => {
            let Some(task) = resolve_task_for_cancel(db, user_id, &query).await? else {
                send_plain_reply(
                    config,
                    channel,
                    thread_ts,
                    "Couldn't find a matching task to cancel.",
                )
                .await?;
                return Ok(());
            };
            db.update_status(task.id, "cancelled").await?;
            db.insert_log(
                task.id,
                "slack",
                "Task cancelled via Slack command",
                "warning",
                None,
            )
            .await?;
            let text = format!("Cancelled task `{}` (`{}`).", task.id, task.title);
            send_plain_reply(config, channel, thread_ts, &text).await?;
        }
    }

    Ok(())
}

async fn resolve_task_for_cancel(
    db: &Database,
    user_id: Uuid,
    query: &str,
) -> Result<Option<AgentTask>> {
    let tasks = db.list_tasks_for_user(user_id).await?;
    if let Ok(id) = Uuid::parse_str(query) {
        return Ok(tasks.into_iter().find(|t| t.id == id));
    }
    if let Ok(number) = query.parse::<i32>() {
        return Ok(tasks
            .into_iter()
            .find(|t| t.task_number.is_some_and(|n| n == number)));
    }
    let needle = query.to_ascii_lowercase();
    Ok(tasks
        .into_iter()
        .find(|t| t.title.to_ascii_lowercase().contains(&needle)))
}

async fn route_thread_feedback(db: &Database, task: &AgentTask, message: &str) -> Result<()> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    db.insert_log(task.id, "user", trimmed, "comment", None)
        .await?;
    db.set_feedback(task.id, trimmed).await?;

    match task.status.as_str() {
        "review" => db.update_status(task.id, "in_progress").await?,
        "plan_review" => db.update_status(task.id, "planning").await?,
        _ => {}
    }
    Ok(())
}

async fn send_plain_reply(
    config: &Config,
    channel: &str,
    thread_ts: &str,
    text: &str,
) -> Result<()> {
    send_message(config, channel, Some(thread_ts), text).await
}

pub async fn send_message(
    config: &Config,
    channel: &str,
    thread_ts: Option<&str>,
    text: &str,
) -> Result<()> {
    let Some(bot_token) = config.slack.bot_token.as_deref() else {
        return Ok(());
    };
    post_message(bot_token, channel, text, thread_ts, None).await?;
    Ok(())
}

fn is_allowlisted_user(config: &Config, slack_user: &str) -> bool {
    config
        .slack
        .allowed_user_ids
        .iter()
        .any(|id| id == slack_user)
}

fn resolve_operator_user_id(config: &Config, slack_user: &str) -> Option<Uuid> {
    let Some(user_id) = config.slack.user_map.get(slack_user).copied() else {
        warn!(slack_user, "Allowlisted Slack user missing user_map entry");
        return None;
    };
    Some(user_id)
}

fn is_operator_surface(event_type: &str, text: &str) -> bool {
    event_type == "app_mention" || starts_with_operator_mention(text)
}

fn starts_with_operator_mention(text: &str) -> bool {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    if let Some(rest) = trimmed.strip_prefix("@karna") {
        return rest.chars().next().is_none_or(is_mention_separator);
    }
    if let Some(rest) = trimmed.strip_prefix("@Karna") {
        return rest.chars().next().is_none_or(is_mention_separator);
    }
    if !trimmed.starts_with("<@") {
        return false;
    }
    let Some(end) = trimmed.find('>') else {
        return false;
    };
    let mention = &trimmed[..=end];
    if mention.len() <= 3 {
        return false;
    }
    let rest = &trimmed[end + 1..];
    rest.chars().next().is_none_or(is_mention_separator)
}

fn is_mention_separator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, ':' | ',' | ';' | '-' | '.')
}

fn resolve_dm_recipient(slack: &crate::config::SlackConfig, task: &AgentTask) -> Option<String> {
    if slack.dm_user_ids.is_empty() {
        return None;
    }
    slack.dm_user_ids.iter().find_map(|slack_user| {
        slack
            .user_map
            .get(slack_user)
            .filter(|&&user_id| user_id == task.user_id)
            .map(|_| slack_user.clone())
    })
}

async fn resolve_notification_channel(
    config: &Config,
    task: &AgentTask,
    bot_token: &str,
) -> Result<Option<String>> {
    if let Some(channel) = task.slack_channel.clone() {
        return Ok(Some(channel));
    }
    if let Some(slack_user) = resolve_dm_recipient(&config.slack, task) {
        let dm_channel = open_direct_message_channel(bot_token, &slack_user).await?;
        return Ok(Some(dm_channel));
    }
    Ok(config.slack.default_channel.clone())
}

fn format_notification(kind: NotificationKind, task: &AgentTask) -> String {
    match kind {
        NotificationKind::PlanReady => format!("Plan ready for *{}*.", task.title),
        NotificationKind::PrOpened => match task.pr_url.as_deref() {
            Some(url) => format!("PR opened for *{}*: {}", task.title, url),
            None => format!("PR opened for *{}*.", task.title),
        },
        NotificationKind::TaskFailed => format!("Task failed: *{}*.", task.title),
        NotificationKind::TaskDone => match task.pr_url.as_deref() {
            Some(url) => format!("Task done: *{}*. {}", task.title, url),
            None => format!("Task done: *{}*.", task.title),
        },
    }
}

fn notification_blocks(kind: NotificationKind, task: &AgentTask) -> Option<serde_json::Value> {
    let task_id = task.id.to_string();
    match kind {
        NotificationKind::PlanReady => Some(json!([
            {
                "type": "actions",
                "elements": [
                    {
                        "type": "button",
                        "text": { "type": "plain_text", "text": "Approve" },
                        "style": "primary",
                        "action_id": format!("karna:gate:plan:approve:{task_id}")
                    },
                    {
                        "type": "button",
                        "text": { "type": "plain_text", "text": "Request changes" },
                        "action_id": format!("karna:gate:plan:request_changes:{task_id}")
                    },
                    {
                        "type": "button",
                        "text": { "type": "plain_text", "text": "Cancel" },
                        "style": "danger",
                        "action_id": format!("karna:gate:plan:cancel:{task_id}")
                    }
                ]
            }
        ])),
        NotificationKind::PrOpened => Some(json!([
            {
                "type": "actions",
                "elements": [
                    {
                        "type": "button",
                        "text": { "type": "plain_text", "text": "Approve" },
                        "style": "primary",
                        "action_id": format!("karna:gate:review:approve:{task_id}")
                    },
                    {
                        "type": "button",
                        "text": { "type": "plain_text", "text": "Request changes" },
                        "action_id": format!("karna:gate:review:request_changes:{task_id}")
                    },
                    {
                        "type": "button",
                        "text": { "type": "plain_text", "text": "Cancel" },
                        "style": "danger",
                        "action_id": format!("karna:gate:review:cancel:{task_id}")
                    }
                ]
            }
        ])),
        NotificationKind::TaskFailed | NotificationKind::TaskDone => None,
    }
}

async fn open_socket_mode_connection(app_token: &str) -> Result<String> {
    let response = reqwest::Client::new()
        .post(SLACK_OPEN_CONNECTION_URL)
        .bearer_auth(app_token)
        .send()
        .await
        .context("apps.connections.open request failed")?;

    let body: SlackOpenConnectionResponse = response
        .json()
        .await
        .context("invalid apps.connections.open response")?;

    if !body.ok {
        anyhow::bail!(
            "apps.connections.open failed: {}",
            body.error.unwrap_or_else(|| "unknown_error".to_string())
        );
    }
    body.url
        .context("apps.connections.open missing websocket URL")
}

async fn open_direct_message_channel(bot_token: &str, slack_user: &str) -> Result<String> {
    let response = reqwest::Client::new()
        .post(SLACK_OPEN_DM_URL)
        .bearer_auth(bot_token)
        .json(&json!({ "users": slack_user }))
        .send()
        .await
        .context("conversations.open request failed")?;

    let body: SlackOpenConversationResponse = response
        .json()
        .await
        .context("invalid conversations.open response")?;

    if !body.ok {
        anyhow::bail!(
            "conversations.open failed: {}",
            body.error.unwrap_or_else(|| "unknown_error".to_string())
        );
    }

    body.channel
        .and_then(|channel| channel.id)
        .context("conversations.open missing channel id")
}

async fn post_message(
    bot_token: &str,
    channel: &str,
    text: &str,
    thread_ts: Option<&str>,
    blocks: Option<serde_json::Value>,
) -> Result<SlackPostMessageResponse> {
    let mut payload = json!({
        "channel": channel,
        "text": text,
    });
    if let Some(ts) = thread_ts {
        payload["thread_ts"] = json!(ts);
    }
    if let Some(blocks) = blocks {
        payload["blocks"] = blocks;
    }

    let response = reqwest::Client::new()
        .post(SLACK_POST_MESSAGE_URL)
        .bearer_auth(bot_token)
        .json(&payload)
        .send()
        .await
        .context("chat.postMessage request failed")?;

    let body: SlackPostMessageResponse = response
        .json()
        .await
        .context("invalid chat.postMessage response")?;

    if !body.ok {
        anyhow::bail!(
            "chat.postMessage failed: {}",
            body.error
                .clone()
                .unwrap_or_else(|| "unknown_error".to_string())
        );
    }
    Ok(body)
}

fn parse_command(text: &str) -> Option<CommandKind> {
    let normalized = normalize_command_text(text);
    if normalized.is_empty() {
        return None;
    }

    if let Some(rest) = normalized.strip_prefix("implement ") {
        let ticket = rest.split_whitespace().next()?.to_uppercase();
        if is_linear_ticket(&ticket) {
            return Some(CommandKind::Implement { ticket });
        }
        return None;
    }
    if normalized == "status" {
        return Some(CommandKind::Status);
    }
    if normalized == "list" {
        return Some(CommandKind::List);
    }
    if let Some(rest) = normalized.strip_prefix("cancel ") {
        let query = rest.trim();
        if !query.is_empty() {
            return Some(CommandKind::Cancel {
                query: query.to_string(),
            });
        }
        return None;
    }
    if let Some(rest) = normalized.strip_prefix("new ") {
        return parse_new_command(rest);
    }

    None
}

fn normalize_command_text(raw: &str) -> String {
    let mention_re = regex_lite::Regex::new(r"<@[^>]+>").expect("static regex");
    let stripped_mentions = mention_re.replace_all(raw, " ");
    let mut text = stripped_mentions.trim().to_string();
    if let Some(rest) = text.strip_prefix("@karna") {
        text = rest.trim().to_string();
    }
    if let Some(rest) = text.strip_prefix("karna") {
        text = rest.trim().to_string();
    }
    text
}

fn parse_new_command(rest: &str) -> Option<CommandKind> {
    let (left, description) = if let Some(parts) = rest.split_once(" — ") {
        parts
    } else {
        rest.split_once(" - ")?
    };
    let left = left.trim();
    let description = description.trim();
    if left.is_empty() || description.is_empty() {
        return None;
    }

    let mut repo: Option<String> = None;
    let mut title = left.to_string();

    if let Some((first, remainder)) = left.split_once(' ') {
        if first.contains('/') && !remainder.trim().is_empty() {
            repo = Some(first.to_string());
            title = remainder.trim().to_string();
        }
    }

    Some(CommandKind::New {
        repo,
        title,
        description: description.to_string(),
    })
}

fn is_linear_ticket(token: &str) -> bool {
    let re = regex_lite::Regex::new(r"^[A-Z][A-Z0-9]+-\d+$").expect("static regex");
    re.is_match(token)
}

fn parse_gate_action(action_id: &str) -> Option<GateAction> {
    let mut parts = action_id.split(':');
    let prefix = parts.next()?;
    let gate = parts.next()?;
    let stage = parts.next()?;
    let decision = parts.next()?;
    let task_id = parts.next()?;
    if prefix != "karna" || gate != "gate" || parts.next().is_some() {
        return None;
    }

    let stage = match stage {
        "plan" => GateStage::Plan,
        "review" => GateStage::Review,
        _ => return None,
    };
    let decision = match decision {
        "approve" => GateDecision::Approve,
        "request_changes" => GateDecision::RequestChanges,
        "cancel" => GateDecision::Cancel,
        _ => return None,
    };

    Some(GateAction {
        stage,
        decision,
        task_id: Uuid::parse_str(task_id).ok()?,
    })
}

#[derive(Debug, Clone)]
enum CommandKind {
    Implement {
        ticket: String,
    },
    New {
        repo: Option<String>,
        title: String,
        description: String,
    },
    Status,
    List,
    Cancel {
        query: String,
    },
}

#[derive(Debug, Clone, Copy)]
enum GateStage {
    Plan,
    Review,
}

#[derive(Debug, Clone, Copy)]
enum GateDecision {
    Approve,
    RequestChanges,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
struct GateAction {
    stage: GateStage,
    decision: GateDecision,
    task_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct SocketEnvelope {
    #[serde(rename = "type")]
    kind: String,
    envelope_id: Option<String>,
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SlackOpenConnectionResponse {
    ok: bool,
    url: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackOpenConversationResponse {
    ok: bool,
    error: Option<String>,
    channel: Option<SlackConversationChannel>,
}

#[derive(Debug, Deserialize)]
struct SlackConversationChannel {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackPostMessageResponse {
    ok: bool,
    error: Option<String>,
    ts: String,
}

#[cfg(test)]
mod tests {
    use super::{
        format_notification, is_operator_surface, parse_command, parse_gate_action,
        resolve_dm_recipient, starts_with_operator_mention, CommandKind, GateDecision, GateStage,
        NotificationKind,
    };
    use crate::config::SlackConfig;
    use crate::models::AgentTask;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn parses_implement_command() {
        let cmd = parse_command("<@U123> implement INFRA-237").expect("expected command");
        match cmd {
            CommandKind::Implement { ticket } => assert_eq!(ticket, "INFRA-237"),
            _ => panic!("unexpected command variant"),
        }
    }

    #[test]
    fn parses_new_command_with_repo() {
        let cmd = parse_command("new owner/repo Add retries — Wire retries into API calls")
            .expect("expected new command");
        match cmd {
            CommandKind::New {
                repo,
                title,
                description,
            } => {
                assert_eq!(repo.as_deref(), Some("owner/repo"));
                assert_eq!(title, "Add retries");
                assert_eq!(description, "Wire retries into API calls");
            }
            _ => panic!("unexpected command variant"),
        }
    }

    #[test]
    fn parses_gate_action_id() {
        let task_id = Uuid::new_v4();
        let action_id = format!("karna:gate:plan:approve:{task_id}");
        let action = parse_gate_action(&action_id).expect("expected gate action");
        assert!(matches!(action.stage, GateStage::Plan));
        assert!(matches!(action.decision, GateDecision::Approve));
        assert_eq!(action.task_id, task_id);
    }

    #[test]
    fn notification_text_includes_pr_url() {
        let mut task = fixture_task();
        task.pr_url = Some("https://github.com/example/repo/pull/12".to_string());
        let message = format_notification(NotificationKind::PrOpened, &task);
        assert!(message.contains("https://github.com/example/repo/pull/12"));
    }

    #[test]
    fn operator_surface_requires_explicit_addressing() {
        assert!(is_operator_surface("app_mention", "status"));
        assert!(is_operator_surface("message", "@karna status"));
        assert!(is_operator_surface("message", "<@U123456> status"));
        assert!(!is_operator_surface("message", "status"));
        assert!(!is_operator_surface("message", "hey <@U123456> status"));
    }

    #[test]
    fn mention_detection_accepts_common_prefixes() {
        assert!(starts_with_operator_mention("@karna: status"));
        assert!(starts_with_operator_mention("@Karna status"));
        assert!(starts_with_operator_mention(
            "<@UABC123>, implement INFRA-1"
        ));
        assert!(!starts_with_operator_mention("prefix @karna status"));
    }

    #[test]
    fn dm_recipient_resolves_only_when_opted_in() {
        let task = fixture_task();
        let mut user_map = HashMap::new();
        user_map.insert("U111".to_string(), task.user_id);
        let slack = SlackConfig {
            enabled: true,
            default_channel: None,
            dm_user_ids: vec!["U111".to_string()],
            allowed_user_ids: vec![],
            user_map,
            bot_token: None,
            app_token: None,
        };
        assert_eq!(resolve_dm_recipient(&slack, &task).as_deref(), Some("U111"));
    }

    fn fixture_task() -> AgentTask {
        AgentTask {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            assignee_user_id: None,
            title: "Test Task".to_string(),
            description: None,
            repo: Some("owner/repo".to_string()),
            kind: "code".to_string(),
            output_target: Some("none".to_string()),
            output_ref: None,
            source: None,
            parent_task_id: None,
            target_branch: None,
            status: "todo".to_string(),
            priority: "medium".to_string(),
            position: 0.0,
            branch: None,
            pr_url: None,
            pr_number: None,
            plan_content: None,
            feedback: None,
            not_before: None,
            agent_session_id: None,
            error_message: None,
            cli: None,
            model: None,
            task_number: Some(1),
            cost_usd: 0.0,
            external_source: None,
            external_id: None,
            external_url: None,
            slack_channel: None,
            slack_thread_ts: None,
            assigned_agent_id: None,
            planner_agent_id: None,
            implementer_agent_id: None,
            reviewer_agent_id: None,
            orchestrator: None,
            policy_matches: None,
            created_at: None,
            updated_at: None,
            started_at: None,
            completed_at: None,
        }
    }
}
