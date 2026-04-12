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

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "todo" => Some(Self::Todo),
            "planning" => Some(Self::Planning),
            "plan_review" => Some(Self::PlanReview),
            "in_progress" => Some(Self::InProgress),
            "review" => Some(Self::Review),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
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

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct AgentTask {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub repo: Option<String>,
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
    pub agent_session_id: Option<String>,
    pub error_message: Option<String>,
    /// CLI backend for this task ("claude", "codex", etc). NULL = use config default.
    pub cli: Option<String>,
    /// Model for this task ("sonnet", "opus", "o4-mini", etc). NULL = use backend default.
    pub model: Option<String>,
    /// Sequential task number per user (auto-assigned by DB trigger).
    pub task_number: Option<i32>,
    /// Accumulated cost in USD across all CLI invocations for this task.
    pub cost_usd: f64,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl AgentTask {
    pub fn status_enum(&self) -> Option<TaskStatus> {
        TaskStatus::from_str(&self.status)
    }

    pub fn target_branch_or_default(&self) -> &str {
        self.target_branch.as_deref().unwrap_or("main")
    }

    /// Generate a branch name like "kar-42/fix-the-bug"
    pub fn agent_branch_name(&self) -> String {
        let number = self.task_number.unwrap_or(0);
        let title_slug = slug::slugify(&self.title);
        let truncated = if title_slug.len() > 40 {
            &title_slug[..40]
        } else {
            &title_slug
        };
        format!("kar-{number}/{truncated}")
    }

    /// Parse comma-separated repos into individual repo refs
    pub fn repos(&self) -> Vec<&str> {
        match &self.repo {
            Some(r) => r.split(',').map(|s| s.trim()).collect(),
            None => Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn is_parent(&self) -> bool {
        // A parent task has no repo set — subtasks carry the repo
        self.repo.is_none() && self.parent_task_id.is_none()
    }

    #[allow(dead_code)]
    pub fn is_subtask(&self) -> bool {
        self.parent_task_id.is_some()
    }
}

#[derive(Debug, Clone, FromRow)]
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

// --- Schedule models ---

#[derive(Debug, Clone, FromRow)]
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
    /// Parse comma-separated repos into individual repo refs.
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

#[derive(Debug, Clone, FromRow)]
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

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct ScheduledRunLog {
    pub id: Uuid,
    pub run_id: Uuid,
    pub level: String,
    pub message: String,
    pub created_at: Option<DateTime<Utc>>,
}

// --- Repo profile models ---

#[derive(Debug, Clone, FromRow, Serialize)]
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
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
