use anyhow::Result;
use indexmap::IndexMap;
use serde::Deserialize;
use std::fs;

/// Lightweight config for the API server — reads the same config.yaml as the agent
/// but only extracts what's needed for the REST API.
#[derive(Clone, Debug)]
pub struct ApiConfig {
    pub repos: Vec<RepoRef>,
    pub backends: IndexMap<String, BackendConfig>,
    pub skills: Vec<SkillRef>,
    pub mcp_servers: Vec<McpServerRef>,
}

#[derive(Clone, Debug)]
pub struct RepoRef {
    pub repo: String,
    pub branch: String,
}

#[derive(Clone, Debug)]
pub struct BackendConfig {
    pub models: Vec<String>,
    pub default_model: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SkillRef {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct McpServerRef {
    pub name: String,
}

// --- Raw YAML structs for deserialization ---

#[derive(Deserialize, Default)]
struct RawConfig {
    #[serde(default)]
    repos: Vec<RawRepo>,
    #[serde(default)]
    agent: RawAgent,
    #[serde(default)]
    skills: Vec<RawSkill>,
    #[serde(default)]
    mcp_servers: Vec<RawMcpServer>,
}

#[derive(Deserialize, Default)]
struct RawAgent {
    #[serde(default)]
    backends: IndexMap<String, RawBackend>,
}

#[derive(Deserialize)]
struct RawRepo {
    repo: String,
    #[serde(default = "default_branch")]
    branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

#[derive(Deserialize)]
struct RawBackend {
    #[serde(default)]
    models: Vec<String>,
    default_model: Option<String>,
}

#[derive(Deserialize)]
struct RawSkill {
    name: String,
}

#[derive(Deserialize)]
struct RawMcpServer {
    name: String,
}

pub fn load() -> Result<ApiConfig> {
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config.yaml".to_string());

    let raw: RawConfig = match fs::read_to_string(&config_path) {
        Ok(content) => serde_yaml::from_str(&content)?,
        Err(_) => {
            tracing::warn!("Config file not found at {config_path}, using defaults");
            RawConfig::default()
        }
    };

    let repos = raw
        .repos
        .into_iter()
        .map(|r| RepoRef {
            repo: r.repo,
            branch: r.branch,
        })
        .collect();

    let backends = raw
        .agent
        .backends
        .into_iter()
        .map(|(name, b)| {
            (
                name,
                BackendConfig {
                    models: b.models,
                    default_model: b.default_model,
                },
            )
        })
        .collect();

    let skills = raw
        .skills
        .into_iter()
        .map(|s| SkillRef { name: s.name })
        .collect();

    let mcp_servers = raw
        .mcp_servers
        .into_iter()
        .map(|s| McpServerRef { name: s.name })
        .collect();

    Ok(ApiConfig {
        repos,
        backends,
        skills,
        mcp_servers,
    })
}
