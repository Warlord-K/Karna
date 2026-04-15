use anyhow::Result;
use tracing::info;

use crate::cli::{self, CliOptions};
use crate::config::{self, Config, Skill};
use crate::db::Database;
use crate::git::workspace;
use crate::models::{AgentTask, RepoProfile, TaskStatus};
use crate::onboarding;

pub async fn plan_task(config: &Config, db: &Database, task: &AgentTask) -> Result<()> {
    info!(task_id = %task.id, "Starting planning");

    db.update_status(task.id, TaskStatus::Planning.as_str()).await?;
    db.insert_log(task.id, "plan", &format!("Starting planning for: {}", task.title), "info", None).await?;

    // Setup workspace — for parent tasks without repo, use all known repos (DB + config)
    let repos_to_use: Vec<String> = if task.repos().is_empty() {
        // DB is source of truth (includes UI-added repos), fall back to config
        let db_repos: Vec<String> = db
            .get_all_repo_profiles()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.repo)
            .collect();
        if db_repos.is_empty() {
            config.repos.iter().map(|r| r.repo.clone()).collect()
        } else {
            db_repos
        }
    } else {
        task.repos().iter().map(|s| s.to_string()).collect()
    };

    // Check for ready repo profiles (smart planning mode)
    let ready_profiles = db.get_ready_repo_profiles().await.unwrap_or_default();
    let has_all_profiles = task.repo.is_none()
        && !ready_profiles.is_empty()
        && repos_to_use.iter().all(|r| ready_profiles.iter().any(|p| p.repo == *r));

    let mut workspace_paths = Vec::new();
    if has_all_profiles {
        // Smart mode: profiles available, only clone first repo for working_dir context
        info!(task_id = %task.id, "Using repo profiles for smart multi-repo planning");
        if let Some(first_repo) = repos_to_use.first() {
            let repo_config = config.find_repo(first_repo);
            let repo_url = repo_config.map(|r| r.repo.as_str()).unwrap_or(first_repo);
            let base_branch = repo_config
                .map(|r| r.branch.as_str())
                .unwrap_or(task.target_branch_or_default());
            let repo_path = workspace::ensure_cloned(&config.repos_dir, repo_url, &config.github_token).await?;
            workspace::checkout_and_pull(&repo_path, base_branch).await?;
            workspace_paths.push(repo_path);
        }
    } else {
        // Fallback: clone all repos (existing behavior)
        for repo_ref in &repos_to_use {
            let repo_config = config.find_repo(repo_ref);
            let repo_url = repo_config.map(|r| r.repo.as_str()).unwrap_or(repo_ref);
            let base_branch = repo_config
                .map(|r| r.branch.as_str())
                .unwrap_or(task.target_branch_or_default());

            let repo_path = workspace::ensure_cloned(&config.repos_dir, repo_url, &config.github_token).await?;
            workspace::checkout_and_pull(&repo_path, base_branch).await?;
            workspace_paths.push(repo_path);
        }
    }

    let working_dir = workspace_paths.first().unwrap_or(&config.repos_dir);

    // Discover repo skills + MCP from all repos
    let (repo_skills, merged_mcp) = discover_all_extensions(config, &workspace_paths).await;

    // Fetch task attachments and write to temp files outside working dir
    let attachments = db.get_task_attachments(task.id).await.unwrap_or_default();
    let images_dir = std::path::PathBuf::from(format!("/tmp/karna-plan-images-{}", task.id));
    let mut image_paths = Vec::new();
    if !attachments.is_empty() {
        tokio::fs::create_dir_all(&images_dir).await.ok();
        for att in &attachments {
            let path = images_dir.join(&att.filename);
            if let Err(e) = tokio::fs::write(&path, &att.data).await {
                tracing::warn!(filename = %att.filename, error = %e, "Failed to write attachment");
                continue;
            }
            image_paths.push(path);
        }
        db.insert_log(task.id, "plan", &format!("Loaded {} image attachment(s)", attachments.len()), "info", None).await?;
    }

    let images_section = if !attachments.is_empty() {
        format!("\n## Attached Images\nThis task includes {} image(s) for visual context. Review them carefully as part of your planning.\n", attachments.len())
    } else {
        String::new()
    };

    // Build prompt
    let description = task.description.as_deref().unwrap_or("No description provided.");
    let repo_display = task.repo.as_deref().unwrap_or("Multiple repositories (see subtask instructions below)");
    let mut prompt = include_str!("../../templates/plan_prompt.txt").to_string();
    prompt = prompt.replace("{title}", &task.title);
    prompt = prompt.replace("{description}", description);
    prompt = prompt.replace("{images_section}", &images_section);
    prompt = prompt.replace("{repo}", repo_display);

    // If no repo is set, this is a potential multi-repo parent task — ask for subtask breakdown
    let subtask_section = if task.repo.is_none() {
        let repo_list: Vec<String> = repos_to_use.iter().map(|r| format!("- `{}`", r)).collect();

        // Inject repo profiles if available
        let profile_context = if has_all_profiles {
            let relevant: Vec<&RepoProfile> = ready_profiles
                .iter()
                .filter(|p| repos_to_use.contains(&p.repo))
                .collect();
            onboarding::format_profiles_for_prompt(&relevant.into_iter().cloned().collect::<Vec<_>>())
        } else {
            String::new()
        };

        format!(
            r#"{profile_context}### Subtask Breakdown (REQUIRED)

This task does not target a specific repository. You MUST break it down into subtasks, one per repository that needs changes. At the end of your plan, include a subtask definition block in exactly this format:

<!-- subtasks
[
  {{"title": "Short subtask title", "repo": "owner/repo", "description": "What this subtask does in this repo"}}
]
subtasks -->

Available repositories:
{}

Rules for subtasks:
- Each subtask MUST have a `repo` field matching one of the available repositories
- Each subtask should be independently implementable
- The title should be concise and specific to the repo's changes
- The description should contain enough context to implement without the parent plan
- Only include repos that actually need changes for this task"#,
            repo_list.join("\n")
        )
    } else {
        String::new()
    };
    prompt = prompt.replace("{subtask_section}", &subtask_section);

    // Inject all skills: global + repo-discovered
    let global_skills = config.skills_for_phase("plan");
    append_skills_to_prompt(&mut prompt, &global_skills, &repo_skills, "plan");

    if let Some(feedback) = &task.feedback {
        prompt.push_str(&format!(
            "\n\n## Previous Plan Feedback\nThe previous plan was rejected with this feedback:\n{feedback}\n\nAddress these concerns in your new plan."
        ));
    }

    let cli_name = task.cli.as_deref().unwrap_or_else(|| config.default_cli());
    let model = task.model.as_deref().unwrap_or_else(|| config.default_model(cli_name));
    db.insert_log(task.id, "plan", &format!("Invoking {cli_name} ({model}) for planning"), "command", None).await?;

    // Write merged MCP config
    let mcp_json_str = merged_mcp.map(|v| v.to_string());

    // Stream CLI events as agent logs so the user can watch progress
    let event_tx = super::spawn_log_consumer(db.clone(), task.id, "plan");

    let result = cli::run(cli_name, CliOptions {
        working_dir,
        prompt: &prompt,
        system_prompt: config.instructions.as_deref(),
        allowed_tools: Some("Read,Glob,Grep,Bash"),
        max_turns: 50,
        model,
        mcp_config_json: mcp_json_str,
        session_id: None,
        resume: false,
        event_tx: Some(event_tx),
        image_paths: image_paths.clone(),
    })
    .await?;

    // Clean up temp image files
    if !image_paths.is_empty() {
        tokio::fs::remove_dir_all(&images_dir).await.ok();
    }

    db.add_cost(task.id, result.cost_usd).await?;

    if let Some(sid) = &result.session_id {
        db.set_session_id(task.id, sid).await?;
    }

    db.set_plan(task.id, &result.output).await?;
    // Only clear feedback if no new comments arrived during planning
    if let Some(current) = db.get_task(task.id).await? {
        let original_feedback = task.feedback.as_deref().unwrap_or("");
        let current_feedback = current.feedback.as_deref().unwrap_or("");
        if current_feedback == original_feedback {
            db.clear_feedback(task.id).await?;
        }
        let _ = crate::notifications::send_plan_ready(config, &current).await;
    }

    db.insert_log(task.id, "plan", "Plan generated, awaiting review", "info", None).await?;
    info!(task_id = %task.id, "Plan ready for review");

    Ok(())
}

/// Discover skills and MCP configs from all workspace repos, merge with global config.
pub async fn discover_all_extensions(
    config: &Config,
    repo_paths: &[std::path::PathBuf],
) -> (Vec<Skill>, Option<serde_json::Value>) {
    let mut repo_skills = Vec::new();
    let mut merged_mcp = config.mcp_config_json();

    for repo_path in repo_paths {
        let extensions = workspace::discover_repo_extensions(repo_path).await;

        // Load skills from repo
        for skill_path in &extensions.skill_paths {
            match config::load_skill_file(skill_path) {
                Ok(skill) => {
                    // Don't add if global config already has a skill with the same name
                    if !config.skills.iter().any(|s| s.name == skill.name)
                        && !repo_skills.iter().any(|s: &Skill| s.name == skill.name)
                    {
                        info!(skill = %skill.name, repo = %repo_path.display(), "Discovered repo skill");
                        repo_skills.push(skill);
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %skill_path.display(), error = %e, "Failed to load repo skill");
                }
            }
        }

        // Merge MCP config from repo
        if let Some(mcp_path) = &extensions.mcp_config_path {
            info!(repo = %repo_path.display(), "Discovered repo MCP config");
            merged_mcp = config::merge_mcp_config(merged_mcp, mcp_path);
        }
    }

    (repo_skills, merged_mcp)
}

/// Append skills section to a prompt, combining global and repo-discovered skills.
pub fn append_skills_to_prompt(
    prompt: &mut String,
    global_skills: &[&Skill],
    repo_skills: &[Skill],
    phase: &str,
) {
    let repo_filtered: Vec<&Skill> = repo_skills
        .iter()
        .filter(|s| s.phase == "both" || s.phase == phase)
        .collect();

    if global_skills.is_empty() && repo_filtered.is_empty() {
        return;
    }

    prompt.push_str("\n\n## Available Skills\n");

    for skill in global_skills.iter().chain(repo_filtered.iter()) {
        prompt.push_str(&format!("\n### Skill: {}\n", skill.name));
        prompt.push_str(&format!("{}\n", skill.description));
        if let Some(cmd) = &skill.command {
            prompt.push_str(&format!("Command: `{cmd}`\n"));
        }
        if let Some(skill_prompt) = &skill.prompt {
            prompt.push_str(&format!("{skill_prompt}\n"));
        }
    }

    if phase == "plan" {
        prompt.push_str("\nIncorporate relevant skills into your plan where appropriate.\n");
    }
}
