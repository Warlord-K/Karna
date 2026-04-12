use anyhow::Result;
use chrono::Utc;
use cron::Schedule as CronSchedule;
use std::str::FromStr;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::agent::planner::{append_skills_to_prompt, discover_all_extensions};
use crate::cli::{self, CliOptions, StreamEvent};
use crate::config::Config;
use crate::db::Database;
use crate::git::workspace;
use crate::models::{Schedule, ScheduledRun};
use crate::queue;

/// Sync config-defined schedules to the database.
/// Inserts any schedules from config.yaml that don't already exist (matched by name).
/// Requires at least one user in the database to assign ownership.
pub async fn sync_config_schedules(config: &Config, db: &Database) -> Result<()> {
    if config.schedules.is_empty() {
        return Ok(());
    }

    let user_id = match db.first_user_id().await? {
        Some(id) => id,
        None => {
            // No users yet — skip seeding, will run on next reload
            return Ok(());
        }
    };

    for sched in &config.schedules {
        if db.schedule_exists_by_name(&sched.name).await? {
            continue;
        }

        info!(schedule = %sched.name, "Syncing config schedule to database");

        db.insert_config_schedule(
            user_id,
            &sched.name,
            &sched.prompt,
            sched.repos.as_deref(),
            sched.cron_expression.as_deref(),
            sched.run_at.as_deref(),
            &sched.skills,
            &sched.mcp_servers,
            sched.max_open_tasks,
            sched.task_prefix.as_deref(),
            &sched.priority,
            sched.cli.as_deref(),
            sched.model.as_deref(),
            sched.enabled,
        )
        .await?;
    }

    Ok(())
}

/// Check all schedules and execute any that are due or manually triggered.
/// Called from the main poll loop every iteration.
pub async fn check_schedules(
    config: &Config,
    db: &Database,
    redis: &redis::Client,
) -> Result<()> {
    let schedules = db.get_all_schedules().await?;
    if schedules.is_empty() {
        return Ok(());
    }

    let worker = queue::worker_id();

    for schedule in schedules {
        let last_run = db.get_last_run(schedule.id).await?;

        // Skip if a run is already in progress
        if last_run.as_ref().is_some_and(|r| r.status == "running") {
            continue;
        }

        // Check if schedule might need to run (trigger or cron-due)
        let trigger_key = format!("schedule_trigger:{}", schedule.id);
        let maybe_triggered = queue::key_exists(redis, &trigger_key).await.unwrap_or(false);

        // Disabled schedules can only run via manual trigger
        if !maybe_triggered && (!schedule.enabled || !is_schedule_due(&schedule, last_run.as_ref())) {
            continue;
        }

        // Acquire lock BEFORE consuming trigger to avoid lost triggers
        let lock_key = format!("schedule_lock:{}", schedule.id);
        if !queue::try_lock_key(redis, &lock_key, &worker).await? {
            continue;
        }

        // Now safe to consume the trigger (we hold the lock)
        let triggered = queue::key_exists(redis, &trigger_key).await.unwrap_or(false);
        if triggered {
            queue::delete_key(redis, &trigger_key).await?;
            info!(schedule = %schedule.name, "Manual trigger consumed");
        }

        // Re-check eligibility under lock (trigger or cron-due)
        if !triggered && !is_schedule_due(&schedule, last_run.as_ref()) {
            queue::release_key(redis, &lock_key).await?;
            continue;
        }

        // Check max_open_tasks
        if schedule.max_open_tasks > 0 {
            if let Some(prefix) = &schedule.task_prefix {
                let open_count = db
                    .count_open_tasks_with_prefix(schedule.user_id, prefix)
                    .await?;
                if open_count >= schedule.max_open_tasks as i64 {
                    info!(
                        schedule = %schedule.name,
                        open = open_count,
                        max = schedule.max_open_tasks,
                        "Skipping schedule: max open tasks reached"
                    );
                    queue::release_key(redis, &lock_key).await?;
                    continue;
                }
            }
        }

        info!(schedule = %schedule.name, "Executing schedule");

        match execute_schedule(config, db, &schedule).await {
            Ok(()) => info!(schedule = %schedule.name, "Schedule completed"),
            Err(e) => error!(schedule = %schedule.name, error = %e, "Schedule failed"),
        }

        // Auto-disable one-shot schedules
        if schedule.is_one_shot() {
            db.disable_schedule(schedule.id).await?;
            info!(schedule = %schedule.name, "One-shot schedule disabled");
        }

        queue::release_key(redis, &lock_key).await?;
    }

    Ok(())
}

/// Determine if a schedule is due based on its cron expression or run_at time.
fn is_schedule_due(schedule: &Schedule, last_run: Option<&ScheduledRun>) -> bool {
    // One-shot: due if run_at is in the past and no completed run exists
    if let Some(run_at) = schedule.run_at {
        return run_at <= Utc::now()
            && last_run.is_none_or(|r| r.status != "completed");
    }

    // Cron: due if next occurrence after last run is in the past
    if let Some(cron_expr) = &schedule.cron_expression {
        let cron_schedule = match CronSchedule::from_str(cron_expr) {
            Ok(s) => s,
            Err(e) => {
                warn!(schedule = %schedule.name, error = %e, "Invalid cron expression");
                return false;
            }
        };

        let last_time = last_run
            .and_then(|r| r.started_at)
            .unwrap_or(chrono::DateTime::UNIX_EPOCH);

        if let Some(next) = cron_schedule.after(&last_time).next() {
            return next <= Utc::now();
        }
    }

    false
}

/// Execute a single schedule: create run, invoke CLI, parse output, create tasks.
async fn execute_schedule(
    config: &Config,
    db: &Database,
    schedule: &Schedule,
) -> Result<()> {
    // Create run record
    let run = db.create_run(schedule.id).await?;
    db.insert_run_log(run.id, "info", &format!("Starting schedule: {}", schedule.name))
        .await?;

    match execute_schedule_inner(config, db, schedule, &run).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let error_msg = format!("{e:#}");
            db.insert_run_log(run.id, "error", &error_msg).await?;
            db.complete_run(run.id, "failed", Some(&error_msg), &[], 0.0)
                .await?;
            Err(e)
        }
    }
}

async fn execute_schedule_inner(
    config: &Config,
    db: &Database,
    schedule: &Schedule,
    run: &ScheduledRun,
) -> Result<()> {
    // Determine which repos to explore
    let repos_to_use: Vec<String> = if schedule.repos().is_empty() {
        config.repos.iter().map(|r| r.repo.clone()).collect()
    } else {
        schedule.repos().iter().map(|s| s.to_string()).collect()
    };

    // Clone/fetch repos
    let mut workspace_paths = Vec::new();
    for repo_ref in &repos_to_use {
        let repo_config = config.find_repo(repo_ref);
        let repo_url = repo_config.map(|r| r.repo.as_str()).unwrap_or(repo_ref);
        let base_branch = repo_config.map(|r| r.branch.as_str()).unwrap_or("main");

        let repo_path =
            workspace::ensure_cloned(&config.repos_dir, repo_url, &config.github_token).await?;
        workspace::checkout_and_pull(&repo_path, base_branch).await?;
        workspace_paths.push(repo_path);
    }

    let working_dir = workspace_paths.first().unwrap_or(&config.repos_dir);

    // Discover repo extensions
    let (repo_skills, merged_mcp) = discover_all_extensions(config, &workspace_paths).await;

    // Build prompt
    let prefix = schedule.task_prefix.as_deref().unwrap_or("SCHED");
    let repos_display = if repos_to_use.is_empty() {
        "All configured repositories".to_string()
    } else {
        repos_to_use.join(", ")
    };

    let mut prompt = include_str!("../templates/schedule_prompt.txt").to_string();
    prompt = prompt.replace("{name}", &schedule.name);
    prompt = prompt.replace("{repos}", &repos_display);
    prompt = prompt.replace("{prompt}", &schedule.prompt);
    prompt = prompt.replace("{prefix}", prefix);

    // Inject skills: global + schedule-requested + repo-discovered
    let global_skills = config.skills_for_phase("plan");
    append_skills_to_prompt(&mut prompt, &global_skills, &repo_skills, "plan");

    // Filter MCP servers: if schedule specifies servers, only use those + global
    // (For now, pass through all merged MCP — the schedule's mcp_servers field is for future filtering)
    let mcp_json_str = merged_mcp.map(|v| v.to_string());

    let cli_name = schedule
        .cli
        .as_deref()
        .unwrap_or_else(|| config.default_cli());
    let model = schedule
        .model
        .as_deref()
        .unwrap_or_else(|| config.default_model(cli_name));

    db.insert_run_log(
        run.id,
        "info",
        &format!("Invoking {cli_name} ({model}) for schedule exploration"),
    )
    .await?;

    // Stream CLI events as run logs
    let event_tx = spawn_run_log_consumer(db.clone(), run.id);

    let result = cli::run(
        cli_name,
        CliOptions {
            working_dir,
            prompt: &prompt,
            system_prompt: config.instructions.as_deref(),
            allowed_tools: Some("Read,Glob,Grep,Bash"),
            max_turns: 50,
            model,
            mcp_config_json: mcp_json_str,
            session_id: None,
            event_tx: Some(event_tx),
        },
    )
    .await?;

    db.insert_run_log(run.id, "info", "CLI execution completed")
        .await?;

    // Parse task directives from output
    let mut created_task_ids: Vec<Uuid> = Vec::new();
    let tasks_to_create = parse_tasks_from_output(&result.output);

    if !tasks_to_create.is_empty() {
        let remaining_slots = if schedule.max_open_tasks > 0 {
            let open = db
                .count_open_tasks_with_prefix(schedule.user_id, prefix)
                .await?;
            (schedule.max_open_tasks as i64 - open).max(0) as usize
        } else {
            tasks_to_create.len()
        };

        for task_def in tasks_to_create.iter().take(remaining_slots) {
            let task = db
                .create_task_from_schedule(
                    schedule.user_id,
                    &task_def.title,
                    Some(&task_def.description),
                    if task_def.repo.is_empty() {
                        None
                    } else {
                        Some(&task_def.repo)
                    },
                    &schedule.priority,
                    schedule.cli.as_deref(),
                    schedule.model.as_deref(),
                )
                .await?;

            info!(
                schedule = %schedule.name,
                task_id = %task.id,
                title = %task_def.title,
                "Created task from schedule"
            );
            db.insert_run_log(
                run.id,
                "info",
                &format!("Created task: {} ({})", task_def.title, task.id),
            )
            .await?;

            created_task_ids.push(task.id);
        }
    }

    // Complete the run
    db.complete_run(
        run.id,
        "completed",
        Some(&result.output),
        &created_task_ids,
        result.cost_usd,
    )
    .await?;

    db.insert_run_log(
        run.id,
        "info",
        &format!(
            "Schedule completed: {} tasks created, ${:.4} cost",
            created_task_ids.len(),
            result.cost_usd
        ),
    )
    .await?;

    Ok(())
}

/// Spawn a background task that consumes CLI stream events and inserts them as run logs.
fn spawn_run_log_consumer(db: Database, run_id: Uuid) -> crate::cli::EventSender {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let (message, level) = match event {
                StreamEvent::ToolUse {
                    tool,
                    input_summary,
                } => {
                    let msg = if input_summary.is_empty() {
                        tool
                    } else {
                        format!("{tool}: {input_summary}")
                    };
                    (msg, "info")
                }
                StreamEvent::AssistantText(text) => {
                    let trimmed = text.trim();
                    if trimmed.len() < 20 {
                        continue;
                    }
                    let truncated: String = trimmed.chars().take(300).collect();
                    (truncated, "info")
                }
                StreamEvent::Error(e) => (format!("Error: {e}"), "error"),
            };
            let _ = db.insert_run_log(run_id, level, &message).await;
        }
    });
    tx
}

#[derive(Debug)]
struct TaskDirective {
    title: String,
    repo: String,
    description: String,
}

/// Parse task definitions from CLI output using the same HTML comment pattern as subtasks.
fn parse_tasks_from_output(output: &str) -> Vec<TaskDirective> {
    let re_match = output
        .find("<!-- tasks")
        .and_then(|start| {
            let after = &output[start..];
            after.find("tasks -->").map(|end| &after[10..end])
        });

    let json_str = match re_match {
        Some(s) => s.trim(),
        None => return Vec::new(),
    };

    let parsed: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "Failed to parse tasks block from schedule output");
            return Vec::new();
        }
    };

    parsed
        .into_iter()
        .filter_map(|v| {
            let title = v.get("title")?.as_str()?.to_string();
            let repo = v.get("repo")?.as_str()?.to_string();
            let description = v
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            Some(TaskDirective {
                title,
                repo,
                description,
            })
        })
        .collect()
}
