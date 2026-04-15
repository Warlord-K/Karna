use anyhow::Result;
use tracing::info;

use crate::cli::{self, CliOptions};
use crate::config::Config;
use crate::db::Database;
use crate::git::{github, workspace};
use crate::models::AgentTask;

use super::planner::{discover_all_extensions, append_skills_to_prompt};

pub async fn implement_task(config: &Config, db: &Database, task: &AgentTask) -> Result<()> {
    info!(task_id = %task.id, "Starting implementation");
    db.insert_log(task.id, "implement", &format!("Starting implementation for: {}", task.title), "info", None).await?;

    let branch_name = task.agent_branch_name();

    // 1. Setup worktree for each repo
    let mut worktree_paths = Vec::new();
    let mut repo_refs: Vec<String> = Vec::new();

    for repo_ref in task.repos() {
        let repo_config = config.find_repo(repo_ref);
        let repo_url = repo_config.map(|r| r.repo.as_str()).unwrap_or(repo_ref);
        let base_branch = repo_config
            .map(|r| r.branch.as_str())
            .unwrap_or(task.target_branch_or_default());

        let repo_path = workspace::ensure_cloned(&config.repos_dir, repo_url, &config.github_token).await?;
        workspace::checkout_and_pull(&repo_path, base_branch).await?;

        let repo_name = repo_url.rsplit('/').next().unwrap_or(repo_ref);
        let worktree_path = config.workspaces_dir.join(task.id.to_string()).join(repo_name);

        workspace::create_worktree(&repo_path, &worktree_path, &branch_name, base_branch).await?;

        worktree_paths.push(worktree_path);
        repo_refs.push(repo_ref.to_string());
    }

    db.set_branch(task.id, &branch_name).await?;

    // 2. Discover repo extensions (skills + MCP)
    let (repo_skills, merged_mcp) = discover_all_extensions(config, &worktree_paths).await;

    // 3. Build implementation prompt with skills
    let working_dir = worktree_paths.first().unwrap();

    // Fetch task attachments and write to temp files outside worktree to avoid git interference
    let attachments = db.get_task_attachments(task.id).await.unwrap_or_default();
    let images_dir = std::path::PathBuf::from(format!("/tmp/karna-images-{}", task.id));
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
        db.insert_log(task.id, "implement", &format!("Loaded {} image attachment(s)", attachments.len()), "info", None).await?;
    }

    let images_section = if !attachments.is_empty() {
        format!("\n## Attached Images\nThis task includes {} image(s) for visual context. Refer to them as needed during implementation.\n", attachments.len())
    } else {
        String::new()
    };

    let description = task.description.as_deref().unwrap_or("No description provided.");
    let plan = task.plan_content.as_deref().unwrap_or("No plan available.");

    let feedback_section = if let Some(feedback) = &task.feedback {
        format!("## Reviewer Feedback (address this)\n{feedback}")
    } else {
        String::new()
    };

    let mut prompt = include_str!("../../templates/implement_prompt.txt").to_string();
    prompt = prompt.replace("{title}", &task.title);
    prompt = prompt.replace("{description}", description);
    prompt = prompt.replace("{images_section}", &images_section);
    prompt = prompt.replace("{plan}", plan);
    prompt = prompt.replace("{feedback_section}", &feedback_section);

    // Inject global + repo-discovered skills
    let global_skills = config.skills_for_phase("implement");
    append_skills_to_prompt(&mut prompt, &global_skills, &repo_skills, "implement");

    // 4. Run CLI backend (Claude Code or Codex)
    let mcp_json_str = merged_mcp.map(|v| v.to_string());
    let cli_name = task.cli.as_deref().unwrap_or_else(|| config.default_cli());
    let model = task.model.as_deref().unwrap_or_else(|| config.default_model(cli_name));

    db.insert_log(task.id, "implement", &format!("Invoking {cli_name} ({model}) for implementation"), "command", None).await?;

    let event_tx = super::spawn_log_consumer(db.clone(), task.id, "implement");

    let result = cli::run(cli_name, CliOptions {
        working_dir,
        prompt: &prompt,
        system_prompt: config.instructions.as_deref(),
        allowed_tools: Some("Read,Write,Edit,Glob,Grep,Bash"),
        max_turns: config.max_turns,
        model,
        mcp_config_json: mcp_json_str,
        session_id: None,
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

    db.insert_log(task.id, "implement", "Claude Code finished, committing changes", "info", None).await?;

    // 5. Commit and push for each worktree
    for (i, worktree_path) in worktree_paths.iter().enumerate() {
        let repo_ref = &repo_refs[i];
        let base_branch = config
            .find_repo(repo_ref)
            .map(|r| r.branch.as_str())
            .unwrap_or(task.target_branch_or_default());

        // Commit any remaining uncommitted changes (Claude Code may have already committed)
        let commit_msg = format!("chore: uncommitted changes from implementation\n\nKarna task: {}", task.id);
        workspace::commit_all(worktree_path, &commit_msg).await?;

        // Push if there are ANY commits on this branch (including ones Claude Code made)
        let has_commits = workspace::has_commits_ahead(worktree_path, base_branch).await?;
        if !has_commits {
            db.insert_log(task.id, "git", "No changes to push — skipping PR", "info", None).await?;
            continue;
        }

        workspace::push(worktree_path, &branch_name).await?;
        db.insert_log(task.id, "git", &format!("Pushed changes to {}", branch_name), "info", None).await?;

        // 6. Create PR — use commit messages for title & body
        let commits = workspace::commit_log_oneline(worktree_path, base_branch)
            .await
            .unwrap_or_default();

        // PR title: first conventional commit subject, or fall back to task title
        let pr_title = commits
            .last() // oldest commit (git log is newest-first)
            .filter(|c| c.contains(':')) // looks like a conventional commit
            .cloned()
            .unwrap_or_else(|| task.title.clone());

        // PR body: description + commit list
        let description = task.description.as_deref().unwrap_or(&task.title);
        let commit_list = if commits.is_empty() {
            String::new()
        } else {
            let items: Vec<String> = commits.iter().map(|c| format!("- {c}")).collect();
            format!("\n\n## Changes\n\n{}", items.join("\n"))
        };

        let pr_body = format!(
            "## Summary\n\n{description}{commit_list}\n\n---\n*Generated by [Karna](https://github.com/Warlord-K/Karna)*"
        );

        let pr = github::create_pr(
            worktree_path,
            repo_ref,
            &branch_name,
            base_branch,
            &pr_title,
            &pr_body,
        )
        .await?;

        db.set_pr(task.id, &pr.url, pr.number).await?;
        db.insert_log(task.id, "git", &format!("PR opened: {}", pr.url), "info", None).await?;
    }

    // Only clear feedback if no new comments arrived during execution.
    // If the user posted a comment mid-run, the poll loop will pick it up.
    if let Some(current) = db.get_task(task.id).await? {
        let original_feedback = task.feedback.as_deref().unwrap_or("");
        let current_feedback = current.feedback.as_deref().unwrap_or("");
        if current_feedback == original_feedback {
            db.clear_feedback(task.id).await?;
        }
        let _ = crate::notifications::send_pr_opened(config, &current).await;
    }

    info!(task_id = %task.id, "Implementation complete, PR opened");
    Ok(())
}

pub async fn apply_feedback(config: &Config, db: &Database, task: &AgentTask) -> Result<()> {
    info!(task_id = %task.id, "Applying feedback to existing PR");
    db.insert_log(task.id, "feedback", "Applying reviewer feedback", "info", None).await?;

    let generated_branch = task.agent_branch_name();
    let branch_name = task.branch.as_deref().unwrap_or(&generated_branch);

    // Gather feedback
    let mut all_feedback = Vec::new();
    if let Some(feedback) = &task.feedback {
        all_feedback.push(format!("From task: {feedback}"));
    }

    if let Some(pr_number) = task.pr_number {
        let repo_ref = task.repos().first().map(|s| s.to_string()).unwrap_or_default();
        let repo_name = repo_ref.rsplit('/').next().unwrap_or(&repo_ref);
        let dir = config.workspaces_dir.join(task.id.to_string()).join(repo_name);
        let fallback_dir = config.repos_dir.join(repo_name);
        let working_dir = if dir.exists() { &dir } else { &fallback_dir };

        if let Ok(comments) = github::get_pr_comments(working_dir, &repo_ref, pr_number).await {
            for c in comments {
                all_feedback.push(format!("From PR review: {c}"));
            }
        }
    }

    let combined_feedback = all_feedback.join("\n\n");

    // Fetch task attachments for visual context during feedback
    let attachments = db.get_task_attachments(task.id).await.unwrap_or_default();
    let feedback_images_dir = std::path::PathBuf::from(format!("/tmp/karna-feedback-images-{}", task.id));
    let mut feedback_image_paths = Vec::new();
    if !attachments.is_empty() {
        tokio::fs::create_dir_all(&feedback_images_dir).await.ok();
        for att in &attachments {
            let path = feedback_images_dir.join(&att.filename);
            if let Err(e) = tokio::fs::write(&path, &att.data).await {
                tracing::warn!(filename = %att.filename, error = %e, "Failed to write attachment");
                continue;
            }
            feedback_image_paths.push(path);
        }
    }

    let description = task.description.as_deref().unwrap_or("");
    let plan = task.plan_content.as_deref().unwrap_or("");

    let mut prompt = format!(
        r#"You are an autonomous coding agent. Apply the following feedback to the existing code changes.

## Task
Title: {title}
Description: {description}

## Original Plan
{plan}

## Feedback to Address
{combined_feedback}

## Rules
- Address ALL feedback items
- Keep changes minimal and focused
- Commit using Conventional Commits: type(scope): description
- NEVER add Co-Authored-By or Signed-off-by trailers to commits
- Do NOT refactor beyond what the feedback requires"#,
        title = task.title,
    );

    // Find worktree
    let repo_ref = task.repos().first().map(|s| s.to_string()).unwrap_or_default();
    let repo_name = repo_ref.rsplit('/').next().unwrap_or(&repo_ref);
    let worktree_path = config.workspaces_dir.join(task.id.to_string()).join(repo_name);

    let working_dir = if worktree_path.exists() {
        &worktree_path
    } else {
        // Re-clone if the repo was lost (ephemeral disk on Render, container restart, etc.)
        let repo_path = config.repos_dir.join(repo_name);
        if !repo_path.exists() {
            workspace::ensure_cloned(&config.repos_dir, &repo_ref, &config.github_token).await?;
        }
        let base_branch = config
            .find_repo(&repo_ref)
            .map(|r| r.branch.as_str())
            .unwrap_or(task.target_branch_or_default());

        workspace::create_worktree(&repo_path, &worktree_path, branch_name, base_branch).await?;
        &worktree_path
    };

    // Discover repo extensions and inject skills
    let (repo_skills, merged_mcp) = discover_all_extensions(config, &[working_dir.to_path_buf()]).await;
    let global_skills = config.skills_for_phase("implement");
    append_skills_to_prompt(&mut prompt, &global_skills, &repo_skills, "implement");

    let mcp_json_str = merged_mcp.map(|v| v.to_string());
    let cli_name = task.cli.as_deref().unwrap_or_else(|| config.default_cli());
    let model = task.model.as_deref().unwrap_or_else(|| config.default_model(cli_name));

    db.insert_log(task.id, "feedback", &format!("Invoking {cli_name} ({model}) to apply feedback (resuming session)"), "command", None).await?;

    let event_tx = super::spawn_log_consumer(db.clone(), task.id, "feedback");

    let result = cli::run(cli_name, CliOptions {
        working_dir,
        prompt: &prompt,
        system_prompt: config.instructions.as_deref(),
        allowed_tools: Some("Read,Write,Edit,Glob,Grep,Bash"),
        max_turns: config.max_turns,
        model,
        mcp_config_json: mcp_json_str,
        session_id: task.agent_session_id.as_deref(),
        event_tx: Some(event_tx),
        image_paths: feedback_image_paths.clone(),
    })
    .await?;

    // Clean up temp image files
    if !feedback_image_paths.is_empty() {
        tokio::fs::remove_dir_all(&feedback_images_dir).await.ok();
    }

    db.add_cost(task.id, result.cost_usd).await?;

    if let Some(sid) = &result.session_id {
        db.set_session_id(task.id, sid).await?;
    }

    let commit_msg = format!("chore: uncommitted changes from feedback\n\nKarna task: {}", task.id);
    let had_changes = workspace::commit_all(working_dir, &commit_msg).await?;
    if had_changes {
        workspace::push(working_dir, branch_name).await?;
    }

    db.update_status(task.id, "review").await?;
    // Only clear feedback if no new comments arrived during execution
    if let Some(current) = db.get_task(task.id).await? {
        let original_feedback = task.feedback.as_deref().unwrap_or("");
        let current_feedback = current.feedback.as_deref().unwrap_or("");
        if current_feedback == original_feedback {
            db.clear_feedback(task.id).await?;
        }
    }
    db.insert_log(task.id, "feedback", "Feedback applied, PR updated", "info", None).await?;

    info!(task_id = %task.id, "Feedback applied, back to review");
    Ok(())
}
