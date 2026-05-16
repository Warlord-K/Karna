//! Seed agent_profiles from config.yaml on startup.
//!
//! Creates one profile per (cli, model) pair declared in `agent.backends`.
//! The default profile is `(config.default_cli, config.default_model_for_default_cli)`.
//! Existing rows (by slug) are left alone — users can rename / repurpose them.

use anyhow::Result;
use tracing::info;

use crate::config::Config;
use crate::db::Database;

/// Build a stable slug for a (cli, model) pair. Lowercased, non-alphanumerics
/// collapsed to dashes. Used as the natural key for the auto-seed.
fn slug_for(cli: &str, model: &str) -> String {
    let raw = format!("{cli}-{model}");
    raw.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Human-friendly name for a (cli, model) pair. "claude" + "sonnet" → "Claude Sonnet".
fn display_name(cli: &str, model: &str) -> String {
    let pretty = |s: &str| {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
            None => String::new(),
        }
    };
    format!("{} {}", pretty(cli), pretty(model))
}

pub async fn seed_from_config(config: &Config, db: &Database) -> Result<()> {
    let default_cli = config.default_cli();
    let default_model = config.default_model(default_cli);

    let mut seeded = 0usize;
    for (cli, backend) in &config.backends {
        for model in &backend.models {
            let slug = slug_for(cli, model);
            let name = display_name(cli, model);
            let is_default = cli == default_cli && model == default_model;
            db.upsert_agent_profile_by_slug(&slug, &name, cli, model, is_default)
                .await?;
            seeded += 1;
        }
    }

    info!(profiles = seeded, "Agent profiles seeded from config");
    Ok(())
}
