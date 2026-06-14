use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    Planning,
    PlanReview,
    InProgress,
    Review,
    Done,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Planning => "planning",
            Self::PlanReview => "plan_review",
            Self::InProgress => "in_progress",
            Self::Review => "review",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug)]
pub struct ParseTaskStatusError;

impl std::str::FromStr for TaskStatus {
    type Err = ParseTaskStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "todo" => Ok(Self::Todo),
            "planning" => Ok(Self::Planning),
            "plan_review" => Ok(Self::PlanReview),
            "in_progress" => Ok(Self::InProgress),
            "review" => Ok(Self::Review),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(ParseTaskStatusError),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

#[allow(dead_code)]
impl TaskPriority {
    pub fn sort_order(&self) -> i32 {
        match self {
            Self::Urgent => 0,
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    #[default]
    Code,
    Doc,
    Research,
    Ops,
}

impl TaskKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Doc => "doc",
            Self::Research => "research",
            Self::Ops => "ops",
        }
    }
}

#[derive(Debug)]
pub struct ParseTaskKindError;

impl std::str::FromStr for TaskKind {
    type Err = ParseTaskKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "code" => Ok(Self::Code),
            "doc" => Ok(Self::Doc),
            "research" => Ok(Self::Research),
            "ops" => Ok(Self::Ops),
            _ => Err(ParseTaskKindError),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskOutputTarget {
    Pr,
    LinearComment,
    LinearDoc,
    SlackMessage,
    Notification,
    #[default]
    None,
}

impl TaskOutputTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pr => "pr",
            Self::LinearComment => "linear_comment",
            Self::LinearDoc => "linear_doc",
            Self::SlackMessage => "slack_message",
            Self::Notification => "notification",
            Self::None => "none",
        }
    }
}

#[derive(Debug)]
pub struct ParseTaskOutputTargetError;

impl std::str::FromStr for TaskOutputTarget {
    type Err = ParseTaskOutputTargetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pr" => Ok(Self::Pr),
            "linear_comment" => Ok(Self::LinearComment),
            "linear_doc" => Ok(Self::LinearDoc),
            "slack_message" => Ok(Self::SlackMessage),
            "notification" => Ok(Self::Notification),
            "none" => Ok(Self::None),
            _ => Err(ParseTaskOutputTargetError),
        }
    }
}

fn default_orchestrator_max_turns() -> u32 {
    12
}

fn default_orchestrator_max_actions_per_turn() -> usize {
    10
}

fn default_orchestrator_max_subtasks() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrchestratorConfig {
    /// MCP tools the orchestrator may request with `run` actions.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default = "default_orchestrator_max_turns")]
    pub max_turns: u32,
    /// Absolute RFC3339 timestamp OR relative duration (e.g. "2h").
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default = "default_orchestrator_max_actions_per_turn")]
    pub max_actions_per_turn: usize,
    #[serde(default = "default_orchestrator_max_subtasks")]
    pub max_subtasks: usize,
    #[serde(default)]
    pub accepts_external_replies: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            allowed_tools: Vec::new(),
            max_turns: default_orchestrator_max_turns(),
            deadline: None,
            max_actions_per_turn: default_orchestrator_max_actions_per_turn(),
            max_subtasks: default_orchestrator_max_subtasks(),
            accepts_external_replies: false,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AgentTask {
    pub id: Uuid,
    pub user_id: Uuid,
    /// NULL = picked up by the agent; set = assigned to a specific human, agent skips.
    pub assignee_user_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub repo: Option<String>,
    /// Task intent: code changes vs non-code workflows.
    pub kind: String,
    /// Where a non-code artifact should be delivered.
    pub output_target: Option<String>,
    /// URL/id/thread-ts of the final artifact destination.
    pub output_ref: Option<String>,
    /// Task origin surface (e.g. "chat"); NULL keeps default board semantics.
    pub source: Option<String>,
    pub parent_task_id: Option<Uuid>,
    pub target_branch: Option<String>,
    pub status: String,
    pub priority: String,
    pub position: f32,
    pub branch: Option<String>,
    pub pr_url: Option<String>,
    pub pr_number: Option<i32>,
    pub plan_content: Option<String>,
    pub feedback: Option<String>,
    /// Gating timestamp for deferred orchestrator turns.
    pub not_before: Option<DateTime<Utc>>,
    pub agent_session_id: Option<String>,
    pub error_message: Option<String>,
    pub cli: Option<String>,
    pub model: Option<String>,
    pub task_number: Option<i32>,
    pub cost_usd: f64,
    /// Origin system if ingested from outside (e.g. "linear", "clickup").
    pub external_source: Option<String>,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    /// Slack thread mapping for control-plane interactions.
    /// When present, task updates are posted into this channel/thread.
    pub slack_channel: Option<String>,
    pub slack_thread_ts: Option<String>,
    /// NULL = any active agent profile may pick it up.
    /// Set = only the named agent profile picks it up.
    pub assigned_agent_id: Option<Uuid>,
    /// Per-stage agent profile overrides for the multi-agent flow. NULL falls
    /// back to `assigned_agent_id`, then the config default (see
    /// `agent::resolve_runtime`). Let a task run scope/implement/review on
    /// different tools/models (e.g. planner=cursor, implementer=codex,
    /// reviewer=grok).
    pub planner_agent_id: Option<Uuid>,
    pub implementer_agent_id: Option<Uuid>,
    pub reviewer_agent_id: Option<Uuid>,
    /// Optional controls for task-level orchestration loops.
    pub orchestrator: Option<serde_json::Value>,
    /// Policies that fired against this task's plan. Shape:
    /// `[{policy_id, name, severity, message, paths: [...]}]`.
    pub policy_matches: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl AgentTask {
    pub fn status_enum(&self) -> Option<TaskStatus> {
        self.status.parse::<TaskStatus>().ok()
    }

    pub fn target_branch_or_default(&self) -> &str {
        self.target_branch.as_deref().unwrap_or("main")
    }

    pub fn kind_enum(&self) -> TaskKind {
        self.kind.parse::<TaskKind>().unwrap_or_default()
    }

    pub fn output_target_enum(&self) -> TaskOutputTarget {
        self.output_target
            .as_deref()
            .unwrap_or(TaskOutputTarget::None.as_str())
            .parse::<TaskOutputTarget>()
            .unwrap_or_default()
    }

    pub fn orchestrator_config(&self) -> Option<OrchestratorConfig> {
        self.orchestrator
            .clone()
            .and_then(|raw| serde_json::from_value::<OrchestratorConfig>(raw).ok())
    }

    pub fn agent_branch_name(&self) -> String {
        let number = self.task_number.unwrap_or(0);

        // Extract prefix from title if it matches "PREFIX-NNN: ..." pattern
        // e.g., "Bug-001: Fix login" → prefix = "Bug", slug of the rest
        let (prefix, slug_source) = if let Some(colon_pos) = self.title.find(':') {
            let before_colon = &self.title[..colon_pos];
            // Check if it's a PREFIX-NNN pattern
            if let Some(dash_pos) = before_colon.rfind('-') {
                let candidate = &before_colon[..dash_pos];
                let after_dash = &before_colon[dash_pos + 1..];
                if !candidate.is_empty()
                    && candidate.chars().all(|c| c.is_alphanumeric() || c == '-')
                    && after_dash.chars().all(|c| c.is_ascii_digit())
                {
                    let rest = self.title[colon_pos + 1..].trim();
                    (candidate.to_lowercase(), rest.to_string())
                } else {
                    ("kar".to_string(), self.title.clone())
                }
            } else {
                ("kar".to_string(), self.title.clone())
            }
        } else {
            ("kar".to_string(), self.title.clone())
        };

        let title_slug = slug::slugify(&slug_source);
        let truncated = if title_slug.len() > 40 {
            &title_slug[..40]
        } else {
            &title_slug
        };
        format!("{prefix}-{number}/{truncated}")
    }

    pub fn repos(&self) -> Vec<&str> {
        match &self.repo {
            Some(r) => r.split(',').map(|s| s.trim()).collect(),
            None => Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn is_parent(&self) -> bool {
        self.repo.is_none() && self.parent_task_id.is_none()
    }

    #[allow(dead_code)]
    pub fn is_subtask(&self) -> bool {
        self.parent_task_id.is_some()
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AgentLog {
    pub id: Uuid,
    pub task_id: Uuid,
    pub phase: String,
    pub message: String,
    pub log_type: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

// --- Task attachment models ---

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct TaskAttachment {
    pub id: Uuid,
    pub task_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub size_bytes: i64,
    pub created_at: Option<DateTime<Utc>>,
}

// --- Schedule models ---

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Schedule {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub prompt: String,
    pub repos: Option<String>,
    pub cron_expression: Option<String>,
    pub run_at: Option<DateTime<Utc>>,
    pub skills: Option<Vec<String>>,
    pub mcp_servers: Option<Vec<String>>,
    pub max_open_tasks: i32,
    pub task_prefix: Option<String>,
    pub priority: String,
    pub cli: Option<String>,
    pub model: Option<String>,
    pub enabled: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Schedule {
    pub fn repos(&self) -> Vec<&str> {
        match &self.repos {
            Some(r) => r.split(',').map(|s| s.trim()).collect(),
            None => Vec::new(),
        }
    }

    pub fn is_one_shot(&self) -> bool {
        self.run_at.is_some()
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ScheduledRun {
    pub id: Uuid,
    pub schedule_id: Uuid,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub summary_markdown: Option<String>,
    pub tasks_created: Option<Vec<Uuid>>,
    pub cost_usd: f64,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ScheduledRunLog {
    pub id: Uuid,
    pub run_id: Uuid,
    pub level: String,
    pub message: String,
    pub created_at: Option<DateTime<Utc>>,
}

// --- Policy models ---

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Policy {
    pub id: Uuid,
    pub name: String,
    /// Repo glob: "owner/repo" exact, "owner/*" prefix, "*" all.
    pub repo_pattern: String,
    /// Path glob (supports `**` and `*`) against files mentioned in plan_content.
    pub path_glob: String,
    pub message: String,
    pub severity: String,
    pub enabled: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// --- Agent profile models ---

/// A named agent identity (e.g. "Sonnet", "Codex GPT-5.4"). Profiles are
/// auto-seeded from config.yaml on agent startup, one per (cli, model) pair,
/// and can be renamed / paused / extended by the user.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub avatar_emoji: String,
    pub cli: String,
    pub model: String,
    pub system_prompt_addendum: Option<String>,
    /// NULL = active. Set = paused with a human-readable reason; the worker
    /// will not claim tasks assigned to this profile while paused.
    pub paused_reason: Option<String>,
    pub is_default: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// --- Repo profile models ---

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RepoProfile {
    pub id: Uuid,
    pub user_id: Uuid,
    pub repo: String,
    pub branch: String,
    pub status: String,
    pub summary: Option<String>,
    pub profile_json: Option<serde_json::Value>,
    pub last_onboarded_at: Option<DateTime<Utc>>,
    pub last_commit_sha: Option<String>,
    pub error_message: Option<String>,
    pub cost_usd: f64,
    pub sync_issues: bool,
    /// not_registered | registered | failed | unsupported
    /// "unsupported" = no public webhook URL configured on the agent.
    pub webhook_status: String,
    pub webhook_error: Option<String>,
    pub webhook_url: Option<String>,
    /// Opt-in: when TRUE, the agent reviews human-opened PRs on this repo.
    pub review_prs: bool,
    /// Which agent profile reviews PRs for this repo. NULL = default agent profile.
    pub review_agent_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// --- PR review records ---

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PrReviewLog {
    pub id: Uuid,
    pub review_id: Uuid,
    pub phase: String,
    pub message: String,
    pub log_type: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PrReview {
    pub id: Uuid,
    pub repo: String,
    pub pr_number: i32,
    pub pr_url: Option<String>,
    pub head_sha: String,
    pub author: Option<String>,
    pub reviewer_agent_id: Option<Uuid>,
    pub status: String,
    pub comments_posted: i32,
    pub cost_usd: f64,
    pub error_message: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PrReviewFinding {
    pub id: Uuid,
    pub review_id: Uuid,
    pub path: String,
    pub line: i32,
    pub start_line: Option<i32>,
    pub side: String,
    pub body: String,
    pub posted: bool,
    pub skip_reason: Option<String>,
    pub severity: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::{TaskKind, TaskOutputTarget};

    #[test]
    fn parses_task_kind_values() {
        assert_eq!("code".parse::<TaskKind>().unwrap().as_str(), "code");
        assert_eq!("doc".parse::<TaskKind>().unwrap().as_str(), "doc");
        assert_eq!("research".parse::<TaskKind>().unwrap().as_str(), "research");
        assert_eq!("ops".parse::<TaskKind>().unwrap().as_str(), "ops");
    }

    #[test]
    fn rejects_invalid_task_kind_values() {
        assert!("invalid".parse::<TaskKind>().is_err());
    }

    #[test]
    fn parses_output_target_values() {
        assert_eq!("pr".parse::<TaskOutputTarget>().unwrap().as_str(), "pr");
        assert_eq!(
            "linear_comment"
                .parse::<TaskOutputTarget>()
                .unwrap()
                .as_str(),
            "linear_comment"
        );
        assert_eq!(
            "linear_doc".parse::<TaskOutputTarget>().unwrap().as_str(),
            "linear_doc"
        );
        assert_eq!(
            "slack_message"
                .parse::<TaskOutputTarget>()
                .unwrap()
                .as_str(),
            "slack_message"
        );
        assert_eq!(
            "notification".parse::<TaskOutputTarget>().unwrap().as_str(),
            "notification"
        );
        assert_eq!("none".parse::<TaskOutputTarget>().unwrap().as_str(), "none");
    }

    #[test]
    fn rejects_invalid_output_target_values() {
        assert!("email".parse::<TaskOutputTarget>().is_err());
    }
}
