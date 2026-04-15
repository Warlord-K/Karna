use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize)]
pub struct BackendConfig {
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub default_model: Option<String>,
}

/// Ordered list of configured backends with their models.
pub type Backends = indexmap::IndexMap<String, BackendConfig>;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub claude_code_oauth_token: Option<String>,
    pub github_token: String,
    pub resend_api_key: Option<String>,
    pub github_webhook_secret: Option<String>,
    /// Public URL for the agent's webhook endpoint (e.g. "https://agent.yourdomain.com").
    /// Derived from: AGENT_WEBHOOK_URL env → TUNNEL_AGENT_HOSTNAME env → None.
    /// When set, webhooks are auto-registered on repos during onboarding.
    pub webhook_url: Option<String>,
    pub notification_email: Option<String>,
    pub from_email: String,
    pub repos_dir: PathBuf,
    pub workspaces_dir: PathBuf,
    pub poll_interval_secs: u64,
    pub max_turns: u32,
    /// Configured backends. First entry is the default.
    pub backends: Backends,
    pub repos: Vec<RepoConfig>,
    pub skills: Vec<Skill>,
    pub mcp_servers: Vec<McpServer>,
    /// User-provided agent instructions (loaded from markdown file).
    /// Injected as system_prompt into every CLI invocation, giving the agent
    /// persistent identity, repo context, and cross-repo conventions.
    pub instructions: Option<String>,
    /// Optional SSH commit signing configuration.
    pub signing: Option<SigningConfig>,
    /// Schedule definitions from config file (synced to DB on startup).
    pub schedules: Vec<ScheduleConfig>,
}

#[derive(Clone, Debug)]
pub struct SigningConfig {
    /// Path to the SSH private key used for signing commits.
    pub ssh_key_path: PathBuf,
    /// Optional path to an allowed_signers file for verification.
    pub allowed_signers_path: Option<PathBuf>,
}

/// Well-known directory where the signing key is mounted (from docker-compose).
const SIGNING_MOUNT_DIR: &str = "/home/agent/.ssh/signing";

/// Common SSH private key filenames to look for in the signing mount.
const SIGNING_KEY_NAMES: &[&str] = &[
    "signing_key",
    "id_ed25519",
    "id_ecdsa",
    "id_rsa",
];

impl SigningConfig {
    /// Scan the well-known signing mount directory for an SSH private key.
    /// Returns None if the directory is empty or doesn't exist.
    fn auto_detect() -> Option<Self> {
        let dir = Path::new(SIGNING_MOUNT_DIR);
        if !dir.is_dir() {
            return None;
        }

        // Look for known key filenames
        for name in SIGNING_KEY_NAMES {
            let key_path = dir.join(name);
            if key_path.is_file() {
                let allowed_signers = dir.join("allowed_signers");
                return Some(Self {
                    ssh_key_path: key_path,
                    allowed_signers_path: if allowed_signers.is_file() {
                        Some(allowed_signers)
                    } else {
                        None
                    },
                });
            }
        }

        None
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RepoConfig {
    /// GitHub repo in "owner/repo" format (e.g. "Warlord-K/backend-kanha")
    pub repo: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    /// Mark this repo as the Karna instance itself for self-iteration.
    /// When set, the agent monitors this repo for updates and triggers
    /// rebuilds (code changes) or hot-reloads (config/skill changes).
    #[serde(default, rename = "self")]
    pub is_self: bool,
}

impl RepoConfig {
    /// Derive a short name from the repo field (e.g. "Warlord-K/backend-kanha" → "backend-kanha")
    pub fn name(&self) -> &str {
        self.repo.rsplit('/').next().unwrap_or(&self.repo)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default = "default_phase")]
    pub phase: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct McpServer {
    pub name: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScheduleConfig {
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub repos: Option<String>,
    #[serde(default)]
    pub cron_expression: Option<String>,
    #[serde(default)]
    pub run_at: Option<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default = "default_max_open_tasks")]
    pub max_open_tasks: i32,
    #[serde(default)]
    pub task_prefix: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default)]
    pub cli: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub enabled: bool,
}

fn default_max_open_tasks() -> i32 {
    3
}
fn default_priority() -> String {
    "medium".to_string()
}
fn default_branch() -> String {
    "main".to_string()
}
fn default_phase() -> String {
    "both".to_string()
}

#[derive(Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    repos: Vec<RepoConfig>,
    #[serde(default)]
    agent: AgentFileConfig,
    #[serde(default)]
    notifications: NotificationsFileConfig,
    #[serde(default)]
    skills: Vec<Skill>,
    #[serde(default)]
    mcp_servers: Vec<McpServer>,
    #[serde(default)]
    signing: Option<SigningFileConfig>,
    #[serde(default)]
    schedules: Vec<ScheduleConfig>,
}

#[derive(Deserialize)]
struct SigningFileConfig {
    ssh_key_path: String,
    #[serde(default)]
    allowed_signers_path: Option<String>,
}

#[derive(Deserialize, Default)]
struct AgentFileConfig {
    #[serde(default = "default_max_turns")]
    max_turns: u32,
    #[serde(default = "default_poll_interval")]
    poll_interval_secs: u64,
    /// Available backends with models. Order matters — first is default.
    #[serde(default = "default_backends")]
    backends: Backends,
    /// Path to a markdown file with agent instructions (identity, repo map, conventions).
    /// Relative paths are resolved from the config file's directory.
    #[serde(default)]
    instructions: Option<String>,
}

#[derive(Deserialize, Default)]
struct NotificationsFileConfig {
    email: Option<String>,
    from_email: Option<String>,
}

fn default_backends() -> Backends {
    let mut backends = Backends::new();
    backends.insert("claude".to_string(), BackendConfig {
        models: vec!["haiku".into(), "sonnet".into(), "opus".into()],
        default_model: Some("sonnet".into()),
    });
    backends.insert("codex".to_string(), BackendConfig {
        models: vec!["o4-mini".into(), "o3".into()],
        default_model: Some("o4-mini".into()),
    });
    backends
}
fn default_max_turns() -> u32 {
    100
}
fn default_poll_interval() -> u64 {
    30
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let config_path = std::env::var("CONFIG_PATH")
            .unwrap_or_else(|_| "/etc/karna/config.yaml".to_string());

        let path = std::path::Path::new(&config_path);
        let file_config = if path.is_file() {
            let contents = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file: {config_path}"))?;
            serde_yaml::from_str::<ConfigFile>(&contents)
                .with_context(|| format!("Failed to parse config file: {config_path}"))?
        } else {
            tracing::warn!("Config file not found at {config_path}, using defaults (repos/schedules from DB)");
            ConfigFile::default()
        };

        let repos_dir = PathBuf::from(
            std::env::var("REPOS_DIR").unwrap_or_else(|_| "/workspace".to_string()),
        );
        let workspaces_dir = PathBuf::from(
            std::env::var("WORKSPACES_DIR")
                .unwrap_or_else(|_| repos_dir.join(".workspaces").to_string_lossy().to_string()),
        );

        // Load skills from config + skills/ directory
        let mut skills = file_config.skills;
        let skills_dir = Path::new(&config_path)
            .parent()
            .unwrap_or(Path::new("."))
            .join("skills");
        if skills_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&skills_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        if let Ok(skill) = load_skill_file(&path) {
                            // Don't add duplicates (config takes precedence)
                            if !skills.iter().any(|s| s.name == skill.name) {
                                skills.push(skill);
                            }
                        }
                    }
                }
            }
        }

        // Load agent instructions file
        let instructions = if let Some(instructions_path) = &file_config.agent.instructions {
            let path = if Path::new(instructions_path).is_absolute() {
                PathBuf::from(instructions_path)
            } else {
                Path::new(&config_path)
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(instructions_path)
            };
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    tracing::info!(path = %path.display(), "Loaded agent instructions");
                    Some(content)
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to load agent instructions file");
                    None
                }
            }
        } else {
            None
        };

        // Resolve env vars in MCP server env
        let mcp_servers: Vec<McpServer> = file_config
            .mcp_servers
            .into_iter()
            .map(|mut server| {
                server.env = server
                    .env
                    .into_iter()
                    .map(|(k, v)| {
                        let resolved = if v.starts_with("${") && v.ends_with('}') {
                            let var_name = &v[2..v.len() - 1];
                            std::env::var(var_name).unwrap_or(v)
                        } else {
                            v
                        };
                        (k, resolved)
                    })
                    .collect();
                server
            })
            .collect();

        // Resolve signing config: explicit config → env var → auto-detect from well-known mount
        let signing = if let Some(sc) = file_config.signing {
            Some(SigningConfig {
                ssh_key_path: PathBuf::from(sc.ssh_key_path),
                allowed_signers_path: sc.allowed_signers_path.map(PathBuf::from),
            })
        } else if let Ok(key_path) = std::env::var("GIT_SIGNING_KEY") {
            Some(SigningConfig {
                ssh_key_path: PathBuf::from(key_path),
                allowed_signers_path: std::env::var("GIT_ALLOWED_SIGNERS").ok().map(PathBuf::from),
            })
        } else {
            // Auto-detect: scan the well-known signing mount directory
            SigningConfig::auto_detect()
        };

        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL is required")?,
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            claude_code_oauth_token: std::env::var("CLAUDE_CODE_OAUTH_TOKEN").ok(),
            github_token: std::env::var("GITHUB_TOKEN")
                .context("GITHUB_TOKEN is required")?,
            resend_api_key: std::env::var("RESEND_API_KEY").ok(),
            github_webhook_secret: std::env::var("GITHUB_WEBHOOK_SECRET").ok(),
            webhook_url: std::env::var("AGENT_WEBHOOK_URL").ok().or_else(|| {
                std::env::var("TUNNEL_AGENT_HOSTNAME").ok().map(|h| format!("https://{h}"))
            }),
            notification_email: file_config
                .notifications
                .email
                .or_else(|| std::env::var("NOTIFICATION_EMAIL").ok()),
            from_email: file_config
                .notifications
                .from_email
                .unwrap_or_else(|| "Karna Agent <agent@karna.dev>".to_string()),
            repos_dir,
            workspaces_dir,
            poll_interval_secs: file_config.agent.poll_interval_secs,
            max_turns: file_config.agent.max_turns,
            backends: file_config.agent.backends,
            repos: file_config.repos,
            skills,
            mcp_servers,
            instructions,
            signing,
            schedules: file_config.schedules,
        })
    }

    /// Default CLI backend name (first in backends map).
    pub fn default_cli(&self) -> &str {
        self.backends.keys().next().map(|s| s.as_str()).unwrap_or("claude")
    }

    /// Default model for a given CLI backend.
    pub fn default_model(&self, cli: &str) -> &str {
        self.backends
            .get(cli)
            .and_then(|b| b.default_model.as_deref().or(b.models.first().map(|s| s.as_str())))
            .unwrap_or("sonnet")
    }

    pub fn find_repo(&self, repo_ref: &str) -> Option<&RepoConfig> {
        self.repos.iter().find(|r| {
            r.repo == repo_ref || r.name() == repo_ref
        })
    }

    /// Get the self-repo config (the Karna instance itself), if configured.
    pub fn self_repo(&self) -> Option<&RepoConfig> {
        self.repos.iter().find(|r| r.is_self)
    }

    /// Get skills applicable to a given phase ("plan" or "implement").
    pub fn skills_for_phase(&self, phase: &str) -> Vec<&Skill> {
        self.skills
            .iter()
            .filter(|s| s.phase == "both" || s.phase == phase)
            .collect()
    }

    /// Generate the MCP config JSON file content for Claude Code.
    /// Returns None if no MCP servers configured.
    pub fn mcp_config_json(&self) -> Option<serde_json::Value> {
        if self.mcp_servers.is_empty() {
            return None;
        }

        let mut servers = serde_json::Map::new();
        for server in &self.mcp_servers {
            let mut entry = serde_json::Map::new();

            if let Some(server_type) = &server.r#type {
                if server_type == "sse" {
                    entry.insert("type".into(), "sse".into());
                    if let Some(url) = &server.url {
                        entry.insert("url".into(), url.clone().into());
                    }
                    servers.insert(server.name.clone(), entry.into());
                    continue;
                }
            }

            if let Some(cmd) = &server.command {
                entry.insert("command".into(), cmd.clone().into());
            }
            if !server.args.is_empty() {
                entry.insert("args".into(), server.args.clone().into());
            }
            if !server.env.is_empty() {
                entry.insert("env".into(), serde_json::json!(server.env));
            }
            servers.insert(server.name.clone(), entry.into());
        }

        Some(serde_json::json!({ "mcpServers": servers }))
    }
}

/// Merge a repo's .mcp.json into the global MCP config.
/// Repo MCP configs use the Claude Code format: { "mcpServers": { ... } }
pub fn merge_mcp_config(
    base: Option<serde_json::Value>,
    repo_mcp_path: &Path,
) -> Option<serde_json::Value> {
    let content = match std::fs::read_to_string(repo_mcp_path) {
        Ok(c) => c,
        Err(_) => return base,
    };

    let repo_config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(path = %repo_mcp_path.display(), error = %e, "Failed to parse repo MCP config");
            return base;
        }
    };

    let mut merged = base.unwrap_or_else(|| serde_json::json!({ "mcpServers": {} }));

    // Merge mcpServers from repo config into the global one
    if let Some(repo_servers) = repo_config.get("mcpServers").and_then(|v| v.as_object()) {
        if let Some(merged_servers) = merged.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
            for (name, server) in repo_servers {
                if !merged_servers.contains_key(name) {
                    merged_servers.insert(name.clone(), server.clone());
                }
            }
        }
    }

    Some(merged)
}

/// Load a skill from a markdown file with YAML frontmatter.
pub fn load_skill_file(path: &Path) -> Result<Skill> {
    let content = std::fs::read_to_string(path)?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Parse frontmatter between --- delimiters
    if let Some(rest) = content.strip_prefix("---") {
        if let Some(idx) = rest.find("---") {
            let frontmatter = &rest[..idx];
            let body = rest[idx + 3..].trim();

            #[derive(Deserialize)]
            struct SkillFrontmatter {
                #[serde(default)]
                description: Option<String>,
                #[serde(default)]
                command: Option<String>,
                #[serde(default = "default_phase")]
                phase: String,
            }

            let fm: SkillFrontmatter = serde_yaml::from_str(frontmatter)?;

            return Ok(Skill {
                name,
                description: fm.description.unwrap_or_default(),
                command: fm.command,
                prompt: if body.is_empty() {
                    None
                } else {
                    Some(body.to_string())
                },
                phase: fm.phase,
            });
        }
    }

    // No frontmatter — entire file is the prompt
    Ok(Skill {
        name,
        description: String::new(),
        command: None,
        prompt: Some(content),
        phase: "both".to_string(),
    })
}
