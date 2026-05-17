use anyhow::Result;
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::cache;
use crate::models::{AgentLog, AgentProfile, AgentTask, PrReview, RepoProfile, Schedule, ScheduledRun, ScheduledRunLog, TaskAttachment};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
    redis: Option<redis::Client>,
    /// When true, user-scoped queries skip the `WHERE user_id = ...` filter.
    /// Every authenticated user sees and can edit every task / schedule / repo.
    /// Auth is still required at the route layer.
    shared_workspace: bool,
}

impl Database {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        Ok(Self { pool, redis: None, shared_workspace: false })
    }

    /// Attach a Redis client so writes automatically invalidate cache keys.
    /// Without this, writes still succeed; only cache busting is skipped.
    pub fn with_redis(mut self, redis: redis::Client) -> Self {
        self.redis = Some(redis);
        self
    }

    /// Enable shared-workspace mode (KARNA_SHARED_WORKSPACE=true). When set,
    /// user-scoped queries return rows regardless of `user_id` so small teams
    /// can collaborate on a single workspace.
    pub fn with_shared_workspace(mut self, shared: bool) -> Self {
        self.shared_workspace = shared;
        self
    }

    pub fn is_shared_workspace(&self) -> bool {
        self.shared_workspace
    }

    // --- Cache invalidation helpers (no-ops when redis is None) ---

    async fn bust_tasks(&self, task_id: Option<Uuid>) {
        let Some(r) = &self.redis else { return };
        cache::invalidate_pattern(r, cache::TASKS_LIST_PATTERN).await;
        if let Some(id) = task_id {
            cache::invalidate(r, &cache::tasks_logs_key(id)).await;
        }
    }

    async fn bust_task_logs(&self, task_id: Uuid) {
        let Some(r) = &self.redis else { return };
        cache::invalidate(r, &cache::tasks_logs_key(task_id)).await;
    }

    async fn bust_schedules(&self, schedule_id: Option<Uuid>) {
        let Some(r) = &self.redis else { return };
        cache::invalidate_pattern(r, cache::SCHEDULES_LIST_PATTERN).await;
        if let Some(id) = schedule_id {
            cache::invalidate(r, &cache::schedule_runs_key(id)).await;
        }
    }

    async fn bust_schedule_run(&self, run_id: Uuid) {
        let Some(r) = &self.redis else { return };
        cache::invalidate(r, &cache::schedule_run_logs_key(run_id)).await;
    }

    async fn bust_repos(&self) {
        let Some(r) = &self.redis else { return };
        cache::invalidate(r, cache::REPOS_LIST_KEY).await;
        // /api/config also embeds repo profiles
        cache::invalidate(r, cache::CONFIG_KEY).await;
    }

    // --- Task queries ---

    pub async fn next_actionable_task(&self) -> Result<Option<AgentTask>> {
        let task = sqlx::query_as::<_, AgentTask>(
            r#"SELECT t.* FROM agent_tasks t
               WHERE t.status IN ('todo', 'planning', 'in_progress')
               AND t.assignee_user_id IS NULL
               AND (
                 t.assigned_agent_id IS NULL
                 OR EXISTS (
                   SELECT 1 FROM agent_profiles p
                   WHERE p.id = t.assigned_agent_id AND p.paused_reason IS NULL
                 )
               )
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

    pub async fn active_task_ids(&self) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT t.id FROM agent_tasks t
               WHERE t.status IN ('planning', 'in_progress')
               AND t.assignee_user_id IS NULL
               AND (
                 t.assigned_agent_id IS NULL
                 OR EXISTS (
                   SELECT 1 FROM agent_profiles p
                   WHERE p.id = t.assigned_agent_id AND p.paused_reason IS NULL
                 )
               )
               AND NOT EXISTS (
                 SELECT 1 FROM agent_tasks sub WHERE sub.parent_task_id = t.id
               )"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn tasks_with_pending_feedback(&self) -> Result<Vec<AgentTask>> {
        let tasks = sqlx::query_as::<_, AgentTask>(
            r#"SELECT t.* FROM agent_tasks t
               WHERE t.status IN ('review', 'plan_review')
               AND t.feedback IS NOT NULL AND t.feedback != ''
               AND t.assignee_user_id IS NULL
               AND (
                 t.assigned_agent_id IS NULL
                 OR EXISTS (
                   SELECT 1 FROM agent_profiles p
                   WHERE p.id = t.assigned_agent_id AND p.paused_reason IS NULL
                 )
               )
               ORDER BY t.updated_at ASC"#,
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

    /// List all tasks for a given user (includes default system user), ordered by priority then creation date.
    /// In shared-workspace mode, returns every task regardless of `user_id`.
    pub async fn list_tasks_for_user(&self, user_id: Uuid) -> Result<Vec<AgentTask>> {
        if self.shared_workspace {
            let tasks = sqlx::query_as::<_, AgentTask>(
                r#"SELECT * FROM agent_tasks
                   ORDER BY
                     CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 END,
                     created_at ASC"#,
            )
            .fetch_all(&self.pool)
            .await?;
            return Ok(tasks);
        }
        let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let tasks = sqlx::query_as::<_, AgentTask>(
            r#"SELECT * FROM agent_tasks
               WHERE user_id = $1 OR user_id = $2
               ORDER BY
                 CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 END,
                 created_at ASC"#,
        )
        .bind(user_id)
        .bind(default_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }

    /// Create a new task for a user.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_task(
        &self,
        user_id: Uuid,
        title: &str,
        description: Option<&str>,
        repo: Option<&str>,
        priority: &str,
        cli: Option<&str>,
        model: Option<&str>,
    ) -> Result<AgentTask> {
        self.create_task_full(
            user_id, title, description, repo, priority, cli, model,
            None, None, None, None, None,
        )
        .await
    }

    /// Create a task with optional assignee and external-source metadata.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_task_full(
        &self,
        user_id: Uuid,
        title: &str,
        description: Option<&str>,
        repo: Option<&str>,
        priority: &str,
        cli: Option<&str>,
        model: Option<&str>,
        assignee_user_id: Option<Uuid>,
        assigned_agent_id: Option<Uuid>,
        external_source: Option<&str>,
        external_id: Option<&str>,
        external_url: Option<&str>,
    ) -> Result<AgentTask> {
        let task = sqlx::query_as::<_, AgentTask>(
            r#"INSERT INTO agent_tasks (
                 user_id, title, description, repo, priority, position, cli, model,
                 assignee_user_id, assigned_agent_id,
                 external_source, external_id, external_url
               )
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
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
        .bind(assignee_user_id)
        .bind(assigned_agent_id)
        .bind(external_source)
        .bind(external_id)
        .bind(external_url)
        .fetch_one(&self.pool)
        .await?;
        self.bust_tasks(Some(task.id)).await;
        Ok(task)
    }

    /// Update allowed fields on a task. Returns number of rows affected.
    pub async fn update_task(
        &self,
        id: Uuid,
        user_id: Uuid,
        updates: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<u64> {
        let allowed = [
            "title", "description", "repo", "target_branch", "status", "priority",
            "position", "branch", "pr_url", "pr_number", "plan_content", "feedback",
            "agent_session_id", "error_message", "cli", "model",
            "assignee_user_id", "assigned_agent_id",
            "external_source", "external_id", "external_url",
        ];

        let mut set_clauses = Vec::new();
        let mut values: Vec<String> = Vec::new();
        let mut idx = 1u32;

        for (key, value) in updates {
            if allowed.contains(&key.as_str()) {
                set_clauses.push(format!("\"{}\" = ${}", key, idx));
                // Store as string for bind — sqlx text bind handles NULL
                values.push(match value {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
                idx += 1;
            }
        }

        if set_clauses.is_empty() {
            return Ok(0);
        }

        // Build a dynamic query — we need to use raw SQL here. In shared-workspace mode
        // the ownership filter is dropped so any signed-in user can edit any task.
        let sql = if self.shared_workspace {
            format!(
                "UPDATE agent_tasks SET {} WHERE id = ${}",
                set_clauses.join(", "),
                idx,
            )
        } else {
            format!(
                "UPDATE agent_tasks SET {} WHERE id = ${} AND (user_id = ${} OR user_id = ${})",
                set_clauses.join(", "),
                idx,
                idx + 1,
                idx + 2,
            )
        };

        let mut query = sqlx::query(&sql);
        for val in &values {
            if val.is_empty() {
                query = query.bind(None::<String>);
            } else {
                query = query.bind(val);
            }
        }
        if self.shared_workspace {
            query = query.bind(id);
        } else {
            let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
            query = query.bind(id).bind(user_id).bind(default_id);
        }

        let result = query.execute(&self.pool).await?;
        if result.rows_affected() > 0 {
            self.bust_tasks(Some(id)).await;
        }
        Ok(result.rows_affected())
    }

    /// Delete a task (must belong to user or default system user, or shared-workspace mode).
    pub async fn delete_task(&self, id: Uuid, user_id: Uuid) -> Result<u64> {
        let result = if self.shared_workspace {
            sqlx::query("DELETE FROM agent_tasks WHERE id = $1")
                .bind(id)
                .execute(&self.pool)
                .await?
        } else {
            let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
            sqlx::query(
                "DELETE FROM agent_tasks WHERE id = $1 AND (user_id = $2 OR user_id = $3)",
            )
            .bind(id)
            .bind(user_id)
            .bind(default_id)
            .execute(&self.pool)
            .await?
        };
        if result.rows_affected() > 0 {
            self.bust_tasks(Some(id)).await;
        }
        Ok(result.rows_affected())
    }

    pub async fn update_status(&self, id: Uuid, status: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.bust_tasks(Some(id)).await;
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
        self.bust_tasks(Some(id)).await;
        Ok(())
    }

    pub async fn set_branch(&self, id: Uuid, branch: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET branch = $1 WHERE id = $2")
            .bind(branch)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.bust_tasks(Some(id)).await;
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
        self.bust_tasks(Some(id)).await;
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
        self.bust_tasks(Some(id)).await;
        Ok(())
    }

    pub async fn clear_feedback(&self, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET feedback = NULL WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.bust_tasks(Some(id)).await;
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
        self.bust_tasks(Some(id)).await;
        Ok(())
    }

    pub async fn set_session_id(&self, id: Uuid, session_id: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET agent_session_id = $1 WHERE id = $2")
            .bind(session_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.bust_tasks(Some(id)).await;
        Ok(())
    }

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
        self.bust_tasks(Some(id)).await;
        Ok(())
    }

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

    #[allow(clippy::too_many_arguments)]
    pub async fn create_subtask(
        &self,
        parent_id: Uuid,
        user_id: Uuid,
        title: &str,
        description: Option<&str>,
        repo: &str,
        priority: &str,
        cli: Option<&str>,
        model: Option<&str>,
    ) -> Result<AgentTask> {
        let task = sqlx::query_as::<_, AgentTask>(
            r#"INSERT INTO agent_tasks (user_id, parent_task_id, title, description, repo, priority, position, cli, model)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING *"#,
        )
        .bind(user_id)
        .bind(parent_id)
        .bind(title)
        .bind(description)
        .bind(repo)
        .bind(priority)
        .bind(Utc::now().timestamp_millis() as f64)
        .bind(cli)
        .bind(model)
        .fetch_one(&self.pool)
        .await?;
        self.bust_tasks(Some(parent_id)).await;
        Ok(task)
    }

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

    #[allow(dead_code)]
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

    // --- Attachment queries ---

    pub async fn get_task_attachments(&self, task_id: Uuid) -> Result<Vec<TaskAttachment>> {
        let attachments = sqlx::query_as::<_, TaskAttachment>(
            "SELECT * FROM task_attachments WHERE task_id = $1 ORDER BY created_at ASC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(attachments)
    }

    /// Copy all attachments from one task to another (used when creating subtasks from a parent).
    pub async fn copy_task_attachments(&self, from_task_id: Uuid, to_task_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            r#"INSERT INTO task_attachments (task_id, filename, content_type, data, size_bytes)
               SELECT $1, filename, content_type, data, size_bytes
               FROM task_attachments WHERE task_id = $2"#,
        )
        .bind(to_task_id)
        .bind(from_task_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
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
        self.bust_task_logs(task_id).await;
        Ok(())
    }

    /// Get recent logs for a task, capped to avoid unbounded growth.
    pub async fn get_logs(&self, task_id: Uuid, limit: i64) -> Result<Vec<AgentLog>> {
        let logs = sqlx::query_as::<_, AgentLog>(
            r#"SELECT * FROM (
                 SELECT * FROM agent_logs WHERE task_id = $1
                 ORDER BY created_at DESC LIMIT $2
               ) sub ORDER BY created_at ASC"#,
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(logs)
    }

    /// Verify a task belongs to a user (or default system user). Returns true if it does.
    /// In shared-workspace mode, returns true whenever the task exists.
    pub async fn task_belongs_to_user(&self, task_id: Uuid, user_id: Uuid) -> Result<bool> {
        if self.shared_workspace {
            let count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM agent_tasks WHERE id = $1",
            )
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
            return Ok(count > 0);
        }
        let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_tasks WHERE id = $1 AND (user_id = $2 OR user_id = $3)",
        )
        .bind(task_id)
        .bind(user_id)
        .bind(default_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    // --- Schedule queries ---

    pub async fn get_all_schedules(&self) -> Result<Vec<Schedule>> {
        let rows = sqlx::query_as::<_, Schedule>(
            "SELECT * FROM schedules ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List schedules for a given user (includes default local user).
    /// In shared-workspace mode, returns every schedule.
    pub async fn list_schedules_for_user(&self, user_id: Uuid) -> Result<Vec<Schedule>> {
        if self.shared_workspace {
            let rows = sqlx::query_as::<_, Schedule>(
                "SELECT * FROM schedules ORDER BY created_at ASC",
            )
            .fetch_all(&self.pool)
            .await?;
            return Ok(rows);
        }
        let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let rows = sqlx::query_as::<_, Schedule>(
            "SELECT * FROM schedules WHERE user_id = $1 OR user_id = $2 ORDER BY created_at ASC",
        )
        .bind(user_id)
        .bind(default_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_schedule(&self, id: Uuid) -> Result<Option<Schedule>> {
        let row = sqlx::query_as::<_, Schedule>(
            "SELECT * FROM schedules WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Create a new schedule.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_schedule(
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
    ) -> Result<Schedule> {
        let row = sqlx::query_as::<_, Schedule>(
            r#"INSERT INTO schedules (
                 user_id, name, prompt, repos, cron_expression, run_at,
                 skills, mcp_servers, max_open_tasks, task_prefix,
                 priority, cli, model
               ) VALUES ($1, $2, $3, $4, $5, $6::timestamptz, $7, $8, $9, $10, $11, $12, $13)
               RETURNING *"#,
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
        .fetch_one(&self.pool)
        .await?;
        self.bust_schedules(None).await;
        Ok(row)
    }

    /// Update a schedule. Returns rows affected.
    pub async fn update_schedule_fields(
        &self,
        id: Uuid,
        user_id: Uuid,
        updates: &serde_json::Value,
    ) -> Result<u64> {
        // Simple approach: update each known field if present
        let obj = match updates.as_object() {
            Some(o) => o,
            None => return Ok(0),
        };

        let mut set_parts = Vec::new();
        let mut bind_idx = 1u32;
        let mut string_vals: Vec<Option<String>> = Vec::new();

        let text_fields = ["name", "prompt", "repos", "cron_expression", "task_prefix", "priority", "cli", "model"];
        for field in &text_fields {
            if let Some(val) = obj.get(*field) {
                set_parts.push(format!("\"{}\" = ${}", field, bind_idx));
                string_vals.push(val.as_str().map(|s| s.to_string()));
                bind_idx += 1;
            }
        }

        if let Some(val) = obj.get("enabled") {
            if let Some(b) = val.as_bool() {
                set_parts.push(format!("enabled = {}", b));
            }
        }

        if let Some(val) = obj.get("max_open_tasks") {
            if let Some(n) = val.as_i64() {
                set_parts.push(format!("max_open_tasks = {}", n));
            }
        }

        if set_parts.is_empty() {
            return Ok(0);
        }

        let sql = if self.shared_workspace {
            format!(
                "UPDATE schedules SET {} WHERE id = ${}",
                set_parts.join(", "),
                bind_idx,
            )
        } else {
            format!(
                "UPDATE schedules SET {} WHERE id = ${} AND (user_id = ${} OR user_id = ${})",
                set_parts.join(", "),
                bind_idx,
                bind_idx + 1,
                bind_idx + 2,
            )
        };

        let mut query = sqlx::query(&sql);
        for val in &string_vals {
            match val {
                Some(s) => query = query.bind(s),
                None => query = query.bind(None::<String>),
            }
        }
        if self.shared_workspace {
            query = query.bind(id);
        } else {
            let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
            query = query.bind(id).bind(user_id).bind(default_id);
        }

        let result = query.execute(&self.pool).await?;
        if result.rows_affected() > 0 {
            self.bust_schedules(Some(id)).await;
        }
        Ok(result.rows_affected())
    }

    /// Delete a schedule (must belong to user, unless shared-workspace mode).
    pub async fn delete_schedule(&self, id: Uuid, user_id: Uuid) -> Result<u64> {
        let result = if self.shared_workspace {
            sqlx::query("DELETE FROM schedules WHERE id = $1")
                .bind(id)
                .execute(&self.pool)
                .await?
        } else {
            let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
            sqlx::query(
                "DELETE FROM schedules WHERE id = $1 AND (user_id = $2 OR user_id = $3)",
            )
            .bind(id)
            .bind(user_id)
            .bind(default_id)
            .execute(&self.pool)
            .await?
        };
        if result.rows_affected() > 0 {
            self.bust_schedules(Some(id)).await;
        }
        Ok(result.rows_affected())
    }

    /// Verify a schedule belongs to a user (or default user, or shared-workspace mode).
    pub async fn schedule_belongs_to_user(&self, id: Uuid, user_id: Uuid) -> Result<bool> {
        if self.shared_workspace {
            let count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM schedules WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
            return Ok(count > 0);
        }
        let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM schedules WHERE id = $1 AND (user_id = $2 OR user_id = $3)",
        )
        .bind(id)
        .bind(user_id)
        .bind(default_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
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
        self.bust_schedules(Some(schedule_id)).await;
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
        // We don't know schedule_id from run_id here; bust list + all runs caches.
        self.bust_schedules(None).await;
        if let Some(r) = &self.redis {
            cache::invalidate_pattern(r, "cache:schedules:runs:*").await;
        }
        self.bust_schedule_run(run_id).await;
        Ok(())
    }

    pub async fn insert_run_log(&self, run_id: Uuid, level: &str, message: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO scheduled_run_logs (run_id, level, message) VALUES ($1, $2, $3)",
        )
        .bind(run_id)
        .bind(level)
        .bind(message)
        .execute(&self.pool)
        .await?;
        self.bust_schedule_run(run_id).await;
        Ok(())
    }

    pub async fn count_open_tasks_with_prefix(&self, user_id: Uuid, prefix: &str) -> Result<i64> {
        let pattern = format!("{prefix}%");
        if self.shared_workspace {
            let count = sqlx::query_scalar::<_, i64>(
                r#"SELECT COUNT(*) FROM agent_tasks
                   WHERE title LIKE $1
                   AND status NOT IN ('done', 'failed', 'cancelled')"#,
            )
            .bind(&pattern)
            .fetch_one(&self.pool)
            .await?;
            return Ok(count);
        }
        let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let count = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM agent_tasks
               WHERE (user_id = $1 OR user_id = $3) AND title LIKE $2
               AND status NOT IN ('done', 'failed', 'cancelled')"#,
        )
        .bind(user_id)
        .bind(&pattern)
        .bind(default_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn max_prefix_number(&self, user_id: Uuid, prefix: &str) -> Result<i32> {
        let pattern = format!("{prefix}-%");
        let titles: Vec<String> = if self.shared_workspace {
            sqlx::query_scalar(
                r#"SELECT title FROM agent_tasks WHERE title LIKE $1"#,
            )
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await?
        } else {
            let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
            sqlx::query_scalar(
                r#"SELECT title FROM agent_tasks
                   WHERE (user_id = $1 OR user_id = $3) AND title LIKE $2"#,
            )
            .bind(user_id)
            .bind(&pattern)
            .bind(default_id)
            .fetch_all(&self.pool)
            .await?
        };

        let prefix_dash = format!("{prefix}-");
        let max_num = titles
            .iter()
            .filter_map(|title| {
                let after = title.strip_prefix(&prefix_dash)?;
                let num_str = after.split(|c: char| !c.is_ascii_digit()).next()?;
                num_str.parse::<i32>().ok()
            })
            .max()
            .unwrap_or(0);

        Ok(max_num)
    }

    #[allow(clippy::too_many_arguments)]
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
        self.bust_tasks(Some(task.id)).await;
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
        self.bust_repos().await;
        Ok(profile)
    }

    pub async fn set_repo_profile_status(&self, id: Uuid, status: &str) -> Result<()> {
        sqlx::query("UPDATE repo_profiles SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.bust_repos().await;
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
        self.bust_repos().await;
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
        self.bust_repos().await;
        Ok(())
    }

    /// Record the outcome of a GitHub webhook registration attempt.
    /// `status` should be one of: `registered`, `failed`, `unsupported`, `not_registered`.
    pub async fn set_repo_webhook_status(
        &self,
        id: Uuid,
        status: &str,
        webhook_url: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE repo_profiles
               SET webhook_status = $1, webhook_url = $2, webhook_error = $3
               WHERE id = $4"#,
        )
        .bind(status)
        .bind(webhook_url)
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.bust_repos().await;
        Ok(())
    }

    pub async fn find_task_by_github_issue(&self, repo: &str, issue_number: i32) -> Result<Option<AgentTask>> {
        let pattern = format!("GH-{}: %", issue_number);
        let task = sqlx::query_as::<_, AgentTask>(
            "SELECT * FROM agent_tasks WHERE repo = $1 AND title LIKE $2 LIMIT 1",
        )
        .bind(repo)
        .bind(&pattern)
        .fetch_optional(&self.pool)
        .await?;
        Ok(task)
    }

    pub async fn get_repo_sync_issues(&self, repo: &str) -> Result<bool> {
        let result = sqlx::query_scalar::<_, bool>(
            "SELECT sync_issues FROM repo_profiles WHERE repo = $1",
        )
        .bind(repo)
        .fetch_optional(&self.pool)
        .await?;
        Ok(result.unwrap_or(true))
    }

    pub async fn update_repo_sync_issues(&self, id: Uuid, sync_issues: bool) -> Result<()> {
        sqlx::query("UPDATE repo_profiles SET sync_issues = $1 WHERE id = $2")
            .bind(sync_issues)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.bust_repos().await;
        Ok(())
    }

    /// Update PR-review opt-in + which agent profile reviews PRs for this repo.
    /// Either field can be passed as `None` to leave it unchanged.
    pub async fn update_repo_review_config(
        &self,
        id: Uuid,
        review_prs: Option<bool>,
        review_agent_id: Option<Option<Uuid>>,
    ) -> Result<u64> {
        let mut set_parts: Vec<String> = Vec::new();
        let mut bind_idx = 1u32;

        if review_prs.is_some() {
            set_parts.push(format!("review_prs = ${bind_idx}"));
            bind_idx += 1;
        }
        if review_agent_id.is_some() {
            set_parts.push(format!("review_agent_id = ${bind_idx}"));
            bind_idx += 1;
        }
        if set_parts.is_empty() {
            return Ok(0);
        }
        let sql = format!(
            "UPDATE repo_profiles SET {} WHERE id = ${}",
            set_parts.join(", "),
            bind_idx,
        );
        let mut query = sqlx::query(&sql);
        if let Some(b) = review_prs {
            query = query.bind(b);
        }
        if let Some(opt) = review_agent_id {
            query = query.bind(opt);
        }
        query = query.bind(id);
        let result = query.execute(&self.pool).await?;
        if result.rows_affected() > 0 {
            self.bust_repos().await;
        }
        Ok(result.rows_affected())
    }

    /// Look up an existing review for a (repo, head_sha) pair. Used to dedupe
    /// before kicking off another review on the same commit.
    pub async fn find_pr_review(&self, repo: &str, head_sha: &str) -> Result<Option<PrReview>> {
        let row = sqlx::query_as::<_, PrReview>(
            "SELECT * FROM pr_reviews WHERE repo = $1 AND head_sha = $2",
        )
        .bind(repo)
        .bind(head_sha)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Insert a `running` review row. Returns `None` if a row for (repo, head_sha)
    /// already exists — race-safe via the UNIQUE constraint — so the caller can
    /// skip cleanly when two webhook firings overlap.
    pub async fn start_pr_review(
        &self,
        repo: &str,
        pr_number: i32,
        pr_url: Option<&str>,
        head_sha: &str,
        author: Option<&str>,
        reviewer_agent_id: Option<Uuid>,
    ) -> Result<Option<PrReview>> {
        let row = sqlx::query_as::<_, PrReview>(
            r#"INSERT INTO pr_reviews
                 (repo, pr_number, pr_url, head_sha, author, reviewer_agent_id, status)
               VALUES ($1, $2, $3, $4, $5, $6, 'running')
               ON CONFLICT (repo, head_sha) DO NOTHING
               RETURNING *"#,
        )
        .bind(repo)
        .bind(pr_number)
        .bind(pr_url)
        .bind(head_sha)
        .bind(author)
        .bind(reviewer_agent_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn complete_pr_review(
        &self,
        id: Uuid,
        status: &str,
        comments_posted: i32,
        cost_usd: f64,
        error_message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE pr_reviews
               SET status = $1, comments_posted = $2, cost_usd = $3,
                   error_message = $4, completed_at = NOW()
               WHERE id = $5"#,
        )
        .bind(status)
        .bind(comments_posted)
        .bind(cost_usd)
        .bind(error_message)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Most recent reviews for a repo — for the eventual repo detail "Reviews" tab.
    pub async fn list_pr_reviews_for_repo(
        &self,
        repo: &str,
        limit: i64,
    ) -> Result<Vec<PrReview>> {
        let rows = sqlx::query_as::<_, PrReview>(
            r#"SELECT * FROM pr_reviews
               WHERE repo = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
        )
        .bind(repo)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete_repo_profile(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM repo_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.bust_repos().await;
        Ok(())
    }

    pub async fn disable_schedule(&self, schedule_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE schedules SET enabled = false WHERE id = $1")
            .bind(schedule_id)
            .execute(&self.pool)
            .await?;
        self.bust_schedules(Some(schedule_id)).await;
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

    pub async fn get_run_logs(&self, run_id: Uuid, limit: i64) -> Result<Vec<ScheduledRunLog>> {
        let logs = sqlx::query_as::<_, ScheduledRunLog>(
            r#"SELECT * FROM (
                 SELECT * FROM scheduled_run_logs WHERE run_id = $1
                 ORDER BY created_at DESC LIMIT $2
               ) sub ORDER BY created_at ASC"#,
        )
        .bind(run_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(logs)
    }

    /// All users, lightest shape needed for the assignee dropdown.
    /// Filters out the synthetic default-system user — it represents "no human", not a person.
    pub async fn list_users(&self) -> Result<Vec<(Uuid, Option<String>, Option<String>)>> {
        let default_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let rows = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>)>(
            r#"SELECT id, name, email FROM users
               WHERE id <> $1
               ORDER BY COALESCE(name, email, '') ASC"#,
        )
        .bind(default_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Lookup an existing task by its origin in Linear/ClickUp — used for webhook dedupe.
    pub async fn find_task_by_external(
        &self,
        source: &str,
        external_id: &str,
    ) -> Result<Option<AgentTask>> {
        let task = sqlx::query_as::<_, AgentTask>(
            "SELECT * FROM agent_tasks WHERE external_source = $1 AND external_id = $2 LIMIT 1",
        )
        .bind(source)
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(task)
    }

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
        let id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn schedule_exists_by_name(&self, name: &str) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM schedules WHERE name = $1",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    // --- Agent profile queries ---

    pub async fn list_agent_profiles(&self) -> Result<Vec<AgentProfile>> {
        let rows = sqlx::query_as::<_, AgentProfile>(
            "SELECT * FROM agent_profiles ORDER BY is_default DESC, name ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_agent_profile(&self, id: Uuid) -> Result<Option<AgentProfile>> {
        let row = sqlx::query_as::<_, AgentProfile>(
            "SELECT * FROM agent_profiles WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Insert a profile if no row with this slug exists. Returns the row in either case.
    /// Used by the agent's startup auto-seed.
    pub async fn upsert_agent_profile_by_slug(
        &self,
        slug: &str,
        name: &str,
        cli: &str,
        model: &str,
        is_default: bool,
    ) -> Result<AgentProfile> {
        // INSERT ... ON CONFLICT DO NOTHING leaves us without RETURNING when the row
        // already exists, so split the seed path: try insert, then fetch.
        let inserted = sqlx::query_as::<_, AgentProfile>(
            r#"INSERT INTO agent_profiles (slug, name, cli, model, is_default)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (slug) DO NOTHING
               RETURNING *"#,
        )
        .bind(slug)
        .bind(name)
        .bind(cli)
        .bind(model)
        .bind(is_default)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = inserted {
            self.bust_agents().await;
            return Ok(row);
        }

        let row = sqlx::query_as::<_, AgentProfile>(
            "SELECT * FROM agent_profiles WHERE slug = $1",
        )
        .bind(slug)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_agent_profile(
        &self,
        slug: &str,
        name: &str,
        cli: &str,
        model: &str,
        avatar_emoji: &str,
        system_prompt_addendum: Option<&str>,
    ) -> Result<AgentProfile> {
        let row = sqlx::query_as::<_, AgentProfile>(
            r#"INSERT INTO agent_profiles
                 (slug, name, cli, model, avatar_emoji, system_prompt_addendum)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING *"#,
        )
        .bind(slug)
        .bind(name)
        .bind(cli)
        .bind(model)
        .bind(avatar_emoji)
        .bind(system_prompt_addendum)
        .fetch_one(&self.pool)
        .await?;
        self.bust_agents().await;
        Ok(row)
    }

    /// Update mutable fields on an agent profile. Whitelisted fields only.
    pub async fn update_agent_profile(
        &self,
        id: Uuid,
        updates: &serde_json::Value,
    ) -> Result<u64> {
        let obj = match updates.as_object() {
            Some(o) => o,
            None => return Ok(0),
        };

        let mut set_parts: Vec<String> = Vec::new();
        let mut bind_idx = 1u32;
        let mut string_vals: Vec<Option<String>> = Vec::new();

        let text_fields = [
            "name",
            "avatar_emoji",
            "cli",
            "model",
            "system_prompt_addendum",
            "paused_reason",
        ];
        for field in &text_fields {
            if let Some(val) = obj.get(*field) {
                set_parts.push(format!("\"{}\" = ${}", field, bind_idx));
                string_vals.push(match val {
                    serde_json::Value::Null => None,
                    serde_json::Value::String(s) => Some(s.clone()),
                    other => Some(other.to_string()),
                });
                bind_idx += 1;
            }
        }

        if let Some(val) = obj.get("is_default") {
            if let Some(b) = val.as_bool() {
                // Demote the previous default first; we can't have two.
                if b {
                    sqlx::query("UPDATE agent_profiles SET is_default = FALSE WHERE is_default = TRUE AND id <> $1")
                        .bind(id)
                        .execute(&self.pool)
                        .await?;
                }
                set_parts.push(format!("is_default = {}", b));
            }
        }

        if set_parts.is_empty() {
            return Ok(0);
        }

        set_parts.push("updated_at = NOW()".to_string());

        let sql = format!(
            "UPDATE agent_profiles SET {} WHERE id = ${}",
            set_parts.join(", "),
            bind_idx,
        );

        let mut query = sqlx::query(&sql);
        for val in &string_vals {
            match val {
                Some(s) => query = query.bind(s),
                None => query = query.bind(None::<String>),
            }
        }
        query = query.bind(id);

        let result = query.execute(&self.pool).await?;
        if result.rows_affected() > 0 {
            self.bust_agents().await;
        }
        Ok(result.rows_affected())
    }

    pub async fn delete_agent_profile(&self, id: Uuid) -> Result<u64> {
        // FK on agent_tasks.assigned_agent_id uses ON DELETE SET NULL — tasks
        // assigned to this profile drop back to "any agent" instead of being deleted.
        let result = sqlx::query("DELETE FROM agent_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() > 0 {
            self.bust_agents().await;
            self.bust_tasks(None).await;
        }
        Ok(result.rows_affected())
    }

    async fn bust_agents(&self) {
        let Some(r) = &self.redis else { return };
        cache::invalidate(r, cache::AGENTS_LIST_KEY).await;
    }

    #[allow(clippy::too_many_arguments)]
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
        self.bust_schedules(None).await;
        Ok(())
    }
}
