use anyhow::Result;
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{AgentLog, AgentTask, RepoProfile, Schedule, ScheduledRun, ScheduledRunLog};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        Ok(Self { pool })
    }

    // --- Task queries ---

    /// Fetch the next task that needs work, ordered by priority then creation date.
    /// Skips parent tasks that have subtasks (those are completed by their children).
    pub async fn next_actionable_task(&self) -> Result<Option<AgentTask>> {
        let task = sqlx::query_as::<_, AgentTask>(
            r#"SELECT t.* FROM agent_tasks t
               WHERE t.status IN ('todo', 'planning', 'in_progress')
               AND NOT EXISTS (
                 SELECT 1 FROM agent_tasks sub WHERE sub.parent_task_id = t.id
               )
               ORDER BY
                 CASE t.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 END,
                 t.created_at ASC
               LIMIT 1"#,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(task)
    }

    /// Return IDs of tasks that appear to be actively worked on (planning or in_progress).
    /// Excludes parent tasks that are in_progress just waiting for subtasks.
    /// Callers should verify Redis locks to confirm a worker actually holds them.
    pub async fn active_task_ids(&self) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT t.id FROM agent_tasks t
               WHERE t.status IN ('planning', 'in_progress')
               AND NOT EXISTS (
                 SELECT 1 FROM agent_tasks sub WHERE sub.parent_task_id = t.id
               )"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Fetch tasks in review or plan_review that have new feedback.
    pub async fn tasks_with_pending_feedback(&self) -> Result<Vec<AgentTask>> {
        let tasks = sqlx::query_as::<_, AgentTask>(
            r#"SELECT * FROM agent_tasks
               WHERE status IN ('review', 'plan_review')
               AND feedback IS NOT NULL AND feedback != ''
               ORDER BY updated_at ASC"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }

    pub async fn get_task(&self, id: Uuid) -> Result<Option<AgentTask>> {
        let task = sqlx::query_as::<_, AgentTask>(
            "SELECT * FROM agent_tasks WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(task)
    }

    pub async fn update_status(&self, id: Uuid, status: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_plan(&self, id: Uuid, plan: &str) -> Result<()> {
        sqlx::query(
            "UPDATE agent_tasks SET plan_content = $1, status = 'plan_review' WHERE id = $2",
        )
        .bind(plan)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_branch(&self, id: Uuid, branch: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET branch = $1 WHERE id = $2")
            .bind(branch)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_pr(&self, id: Uuid, pr_url: &str, pr_number: i32) -> Result<()> {
        sqlx::query(
            "UPDATE agent_tasks SET pr_url = $1, pr_number = $2, status = 'review' WHERE id = $3",
        )
        .bind(pr_url)
        .bind(pr_number)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_error(&self, id: Uuid, error_message: &str) -> Result<()> {
        sqlx::query(
            "UPDATE agent_tasks SET error_message = $1, status = 'failed' WHERE id = $2",
        )
        .bind(error_message)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_feedback(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET feedback = NULL WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_cost(&self, id: Uuid, amount: f64) -> Result<()> {
        if amount <= 0.0 {
            return Ok(());
        }
        sqlx::query("UPDATE agent_tasks SET cost_usd = cost_usd + $1 WHERE id = $2")
            .bind(amount)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_session_id(&self, id: Uuid, session_id: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET agent_session_id = $1 WHERE id = $2")
            .bind(session_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Find a task by its branch name (used by GitHub webhooks).
    pub async fn find_task_by_branch(&self, branch: &str) -> Result<Option<AgentTask>> {
        let task = sqlx::query_as::<_, AgentTask>(
            "SELECT * FROM agent_tasks WHERE branch = $1 LIMIT 1",
        )
        .bind(branch)
        .fetch_optional(&self.pool)
        .await?;
        Ok(task)
    }

    pub async fn set_feedback(&self, id: Uuid, feedback: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET feedback = $1 WHERE id = $2")
            .bind(feedback)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get tasks that are done or failed for longer than the given hours (for cleanup).
    pub async fn stale_completed_tasks(&self, older_than_hours: i64) -> Result<Vec<AgentTask>> {
        let tasks = sqlx::query_as::<_, AgentTask>(
            r#"SELECT * FROM agent_tasks
               WHERE status IN ('done', 'failed', 'cancelled')
               AND updated_at < NOW() - make_interval(hours => $1::int)
               AND branch IS NOT NULL"#,
        )
        .bind(older_than_hours)
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }

    // --- Subtask queries ---

    /// Create a subtask under a parent task.
    pub async fn create_subtask(
        &self,
        parent_id: Uuid,
        user_id: Uuid,
        title: &str,
        description: Option<&str>,
        repo: &str,
        priority: &str,
    ) -> Result<AgentTask> {
        let task = sqlx::query_as::<_, AgentTask>(
            r#"INSERT INTO agent_tasks (user_id, parent_task_id, title, description, repo, priority, position)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING *"#,
        )
        .bind(user_id)
        .bind(parent_id)
        .bind(title)
        .bind(description)
        .bind(repo)
        .bind(priority)
        .bind(Utc::now().timestamp_millis() as f64)
        .fetch_one(&self.pool)
        .await?;
        Ok(task)
    }

    /// Fetch all subtasks for a parent task.
    pub async fn get_subtasks(&self, parent_id: Uuid) -> Result<Vec<AgentTask>> {
        let tasks = sqlx::query_as::<_, AgentTask>(
            r#"SELECT * FROM agent_tasks
               WHERE parent_task_id = $1
               ORDER BY position ASC, created_at ASC"#,
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }

    /// Check if all subtasks of a parent are done and update parent status accordingly.
    pub async fn check_parent_completion(&self, parent_id: Uuid) -> Result<bool> {
        let row = sqlx::query_as::<_, (i64, i64)>(
            r#"SELECT COUNT(*),
                      COUNT(*) FILTER (WHERE status = 'done')
               FROM agent_tasks WHERE parent_task_id = $1"#,
        )
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;

        let (total, done) = row;
        if total > 0 && total == done {
            self.update_status(parent_id, "done").await?;
            return Ok(true);
        }
        Ok(false)
    }

    // --- Log queries ---

    pub async fn insert_log(
        &self,
        task_id: Uuid,
        phase: &str,
        message: &str,
        log_type: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO agent_logs (task_id, phase, message, log_type, metadata)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(task_id)
        .bind(phase)
        .bind(message)
        .bind(log_type)
        .bind(metadata)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_logs(&self, task_id: Uuid) -> Result<Vec<AgentLog>> {
        let logs = sqlx::query_as::<_, AgentLog>(
            "SELECT * FROM agent_logs WHERE task_id = $1 ORDER BY created_at ASC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(logs)
    }

    // --- Schedule queries ---

    pub async fn get_enabled_schedules(&self) -> Result<Vec<Schedule>> {
        let rows = sqlx::query_as::<_, Schedule>(
            "SELECT * FROM schedules WHERE enabled = true ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_last_run(&self, schedule_id: Uuid) -> Result<Option<ScheduledRun>> {
        let run = sqlx::query_as::<_, ScheduledRun>(
            "SELECT * FROM scheduled_runs WHERE schedule_id = $1 ORDER BY started_at DESC LIMIT 1",
        )
        .bind(schedule_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(run)
    }

    pub async fn create_run(&self, schedule_id: Uuid) -> Result<ScheduledRun> {
        let run = sqlx::query_as::<_, ScheduledRun>(
            "INSERT INTO scheduled_runs (schedule_id) VALUES ($1) RETURNING *",
        )
        .bind(schedule_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(run)
    }

    pub async fn complete_run(
        &self,
        run_id: Uuid,
        status: &str,
        summary: Option<&str>,
        tasks_created: &[Uuid],
        cost_usd: f64,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE scheduled_runs
               SET status = $1, summary_markdown = $2, tasks_created = $3,
                   cost_usd = $4, completed_at = NOW()
               WHERE id = $5"#,
        )
        .bind(status)
        .bind(summary)
        .bind(tasks_created)
        .bind(cost_usd)
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_run_log(
        &self,
        run_id: Uuid,
        level: &str,
        message: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO scheduled_run_logs (run_id, level, message) VALUES ($1, $2, $3)",
        )
        .bind(run_id)
        .bind(level)
        .bind(message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Count open tasks with a given title prefix (not done/failed).
    pub async fn count_open_tasks_with_prefix(&self, user_id: Uuid, prefix: &str) -> Result<i64> {
        let pattern = format!("{prefix}%");
        let count = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM agent_tasks
               WHERE user_id = $1 AND title LIKE $2
               AND status NOT IN ('done', 'failed')"#,
        )
        .bind(user_id)
        .bind(&pattern)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// Create a task from a schedule run.
    pub async fn create_task_from_schedule(
        &self,
        user_id: Uuid,
        title: &str,
        description: Option<&str>,
        repo: Option<&str>,
        priority: &str,
        cli: Option<&str>,
        model: Option<&str>,
    ) -> Result<AgentTask> {
        let task = sqlx::query_as::<_, AgentTask>(
            r#"INSERT INTO agent_tasks (user_id, title, description, repo, priority, position, cli, model)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING *"#,
        )
        .bind(user_id)
        .bind(title)
        .bind(description)
        .bind(repo)
        .bind(priority)
        .bind(Utc::now().timestamp_millis() as f64)
        .bind(cli)
        .bind(model)
        .fetch_one(&self.pool)
        .await?;
        Ok(task)
    }

    // --- Repo profile queries ---

    pub async fn get_repo_profile(&self, repo: &str) -> Result<Option<RepoProfile>> {
        let profile = sqlx::query_as::<_, RepoProfile>(
            "SELECT * FROM repo_profiles WHERE repo = $1",
        )
        .bind(repo)
        .fetch_optional(&self.pool)
        .await?;
        Ok(profile)
    }

    pub async fn get_all_repo_profiles(&self) -> Result<Vec<RepoProfile>> {
        let profiles = sqlx::query_as::<_, RepoProfile>(
            "SELECT * FROM repo_profiles ORDER BY repo ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(profiles)
    }

    pub async fn get_ready_repo_profiles(&self) -> Result<Vec<RepoProfile>> {
        let profiles = sqlx::query_as::<_, RepoProfile>(
            "SELECT * FROM repo_profiles WHERE status = 'ready' ORDER BY repo ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(profiles)
    }

    pub async fn upsert_repo_profile(&self, user_id: Uuid, repo: &str, branch: &str) -> Result<RepoProfile> {
        let profile = sqlx::query_as::<_, RepoProfile>(
            r#"INSERT INTO repo_profiles (user_id, repo, branch, status)
               VALUES ($1, $2, $3, 'pending')
               ON CONFLICT (repo) DO UPDATE SET
                 branch = EXCLUDED.branch,
                 status = CASE
                   WHEN repo_profiles.status IN ('ready', 'stale', 'failed') THEN 'pending'
                   ELSE repo_profiles.status
                 END
               RETURNING *"#,
        )
        .bind(user_id)
        .bind(repo)
        .bind(branch)
        .fetch_one(&self.pool)
        .await?;
        Ok(profile)
    }

    pub async fn set_repo_profile_status(&self, id: Uuid, status: &str) -> Result<()> {
        sqlx::query("UPDATE repo_profiles SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_repo_profile_data(
        &self,
        id: Uuid,
        summary: &str,
        profile_json: serde_json::Value,
        commit_sha: &str,
        cost_usd: f64,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE repo_profiles
               SET summary = $1, profile_json = $2, last_commit_sha = $3,
                   cost_usd = cost_usd + $4, status = 'ready',
                   last_onboarded_at = NOW(), error_message = NULL
               WHERE id = $5"#,
        )
        .bind(summary)
        .bind(profile_json)
        .bind(commit_sha)
        .bind(cost_usd)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_repo_profile_error(&self, id: Uuid, error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE repo_profiles SET status = 'failed', error_message = $1 WHERE id = $2",
        )
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_repo_profile(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM repo_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn disable_schedule(&self, schedule_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE schedules SET enabled = false WHERE id = $1")
            .bind(schedule_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_schedule_runs(&self, schedule_id: Uuid, limit: i64) -> Result<Vec<ScheduledRun>> {
        let runs = sqlx::query_as::<_, ScheduledRun>(
            "SELECT * FROM scheduled_runs WHERE schedule_id = $1 ORDER BY started_at DESC LIMIT $2",
        )
        .bind(schedule_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(runs)
    }

    pub async fn get_run_logs(&self, run_id: Uuid) -> Result<Vec<ScheduledRunLog>> {
        let logs = sqlx::query_as::<_, ScheduledRunLog>(
            "SELECT * FROM scheduled_run_logs WHERE run_id = $1 ORDER BY created_at ASC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(logs)
    }

    /// Get the default user ID for config-synced records.
    /// Prefers the well-known local user (00000000-...) to stay consistent with
    /// the frontend's DEFAULT_USER_ID, falling back to any user if it doesn't exist.
    pub async fn first_user_id(&self) -> Result<Option<Uuid>> {
        let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)",
        )
        .bind(default_id)
        .fetch_one(&self.pool)
        .await?;
        if exists {
            return Ok(Some(default_id));
        }
        let id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM users LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    /// Check if a schedule with the given name exists (any user).
    pub async fn schedule_exists_by_name(&self, name: &str) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM schedules WHERE name = $1",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    /// Insert a schedule from config.
    pub async fn insert_config_schedule(
        &self,
        user_id: Uuid,
        name: &str,
        prompt: &str,
        repos: Option<&str>,
        cron_expression: Option<&str>,
        run_at: Option<&str>,
        skills: &[String],
        mcp_servers: &[String],
        max_open_tasks: i32,
        task_prefix: Option<&str>,
        priority: &str,
        cli: Option<&str>,
        model: Option<&str>,
        enabled: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO schedules (
                 user_id, name, prompt, repos, cron_expression, run_at,
                 skills, mcp_servers, max_open_tasks, task_prefix,
                 priority, cli, model, enabled
               ) VALUES ($1, $2, $3, $4, $5, $6::timestamptz, $7, $8, $9, $10, $11, $12, $13, $14)"#,
        )
        .bind(user_id)
        .bind(name)
        .bind(prompt)
        .bind(repos)
        .bind(cron_expression)
        .bind(run_at)
        .bind(skills)
        .bind(mcp_servers)
        .bind(max_open_tasks)
        .bind(task_prefix)
        .bind(priority)
        .bind(cli)
        .bind(model)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
