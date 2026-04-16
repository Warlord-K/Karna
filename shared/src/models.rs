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

#[derive(Debug, Clone, FromRow, Serialize)]
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
    pub cli: Option<String>,
    pub model: Option<String>,
    pub task_number: Option<i32>,
    pub cost_usd: f64,
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

#[derive(Debug, Clone, FromRow, Serialize)]
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

#[derive(Debug, Clone, FromRow, Serialize)]
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

#[derive(Debug, Clone, FromRow, Serialize)]
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

#[derive(Debug, Clone, FromRow, Serialize)]
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
