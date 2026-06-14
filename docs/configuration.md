# Karna Configuration and Operations

This is an operator/developer reference for running Karna as currently implemented.

## Configuration Sources

Karna reads settings from:

1. Environment variables
2. `config.yaml` (path from `CONFIG_PATH`, default `/etc/karna/config.yaml`)
3. Runtime DB state (for schedules/repo profiles/tasks)

The agent and API both read the same `config.yaml` path, but the agent uses the full schema while API reads a lightweight subset.

## `config.yaml` Reference

The canonical shape is shown in `config.example.yaml`.

## `repos`

```yaml
repos:
  - repo: owner/repo
    branch: main
    sync_issues: true
    self: false
```

- `repo`: GitHub repo in `owner/repo` format
- `branch`: base branch for planning/worktrees (default `main`)
- `sync_issues`: if true, new GitHub issues can be ingested as tasks
- `self`: marks Karna's own repo for self-update checks

## `agent`

```yaml
agent:
  max_turns: 100
  poll_interval_secs: 30
  max_review_rounds: 3
  instructions: instructions.md
  backends:
    claude:
      models: [opus, sonnet, haiku]
      default_model: sonnet
```

- `max_turns`: max CLI turns for implement/feedback/generic runs
- `poll_interval_secs`: worker poll interval
- `max_review_rounds`: implement <-> self-review loops (`0` disables self-review)
- `instructions`: markdown file loaded and injected as system prompt context
- `backends`: ordered backend map; first key is default CLI

Notes:

- `instructions` path resolves relative to the config file directory unless absolute.
- Repo/local `skills/*.md` and repo `.mcp.json` are discovered at runtime and merged into prompts/MCP config.

## `memory`

```yaml
memory:
  enabled: false
  url: http://localhost:8888
  max_items: 8
  max_chars: 2000
```

- `enabled`: toggle mem0 usage
- `url`: mem0 API base URL
- `max_items`: max snippets injected
- `max_chars`: budget for injected `## Memory` section

### mem0 provider configuration (Docker/Helm)

Karna's `memory` feature is only an HTTP client. LLM + embedding inference is done by the mem0 server container, configured via mem0 env vars.

Default behavior now routes both mem0 chat + embeddings through OpenRouter's OpenAI-compatible endpoint:

- `LLM_PROVIDER=openai`
- `LLM_MODEL=openai/gpt-4o-mini`
- `LLM_BASE_URL=https://openrouter.ai/api/v1`
- `EMBEDDER_PROVIDER=openai`
- `EMBEDDER_MODEL=openai/text-embedding-3-small`
- `EMBEDDER_BASE_URL=https://openrouter.ai/api/v1`

Notes:

- `openai/text-embedding-3-small` is a 1536-dimensional embedding model in mem0/OpenAI-compatible flows.
- API keys default to `OPENROUTER_API_KEY` (compose) or `openrouter.*` Helm secret values; Helm can override mem0-specific routing with `mem0.openrouterApiKey` / `mem0.openrouterExistingSecret`.
- Everything remains overrideable for non-OpenRouter setups (OpenAI/Ollama/etc.) by changing provider/model/base URL and key source.

## `slack`

```yaml
slack:
  enabled: false
  default_channel: C0123456789
  dm_user_ids: []
  allowed_user_ids: []
  user_map:
    U0123456789: 00000000-0000-0000-0000-000000000000
```

- `enabled`: enables Slack notifications + Socket Mode command surface
- `default_channel`: fallback channel when task has no mapped thread
- `dm_user_ids`: optional DM notification targets (matched via `user_map`)
- `allowed_user_ids`: allowlist for command surface
- `user_map`: Slack user ID -> Karna user UUID mapping

## `mcp_servers`

```yaml
mcp_servers:
  - name: linear
    command: npx
    args: ["-y", "@linear/mcp-server"]
    env:
      LINEAR_API_KEY: "${LINEAR_API_KEY}"
```

`${VAR}` placeholder behavior (implemented in `agent/src/config.rs`):

- if value is exactly `${VAR}` and env var exists -> replaced with env var value
- if env var is missing -> placeholder string is kept as-is

This is why secrets should be delivered through environment variables/Secrets, not literal tokens in ConfigMaps.

## `instructions`

The instructions file content is loaded once into config and merged into each stage prompt (plus optional per-profile addendum).

## `schedules`

```yaml
schedules:
  - name: Bug Hunter
    prompt: "...markdown..."
    cron_expression: "0 */6 * * *"
    task_prefix: BUG
    max_open_tasks: 3
    priority: high
    enabled: false
```

Key points:

- config schedules are seeded into DB by name
- if schedule already exists in DB, config does not overwrite it
- after seeding, DB row is operational source of truth
- one-shot schedules use `run_at`, then auto-disable after running

## `signing`

```yaml
signing:
  ssh_key_path: /path/to/key
  allowed_signers_path: /path/to/allowed_signers
```

Resolution order:

1. explicit `signing` block in config
2. env vars `GIT_SIGNING_KEY` / `GIT_ALLOWED_SIGNERS`
3. auto-detect in `/home/agent/.ssh/signing` (`signing_key`, `id_ed25519`, etc.)

## Secrets: File vs Environment

### Comes from `config.yaml`

- repo list and branches
- backend/model catalog
- memory/slack toggles and non-secret settings
- MCP server definitions (with placeholders)
- schedules
- instructions path

### Comes from environment / Kubernetes Secret

Required for worker startup:

- `DATABASE_URL`
- `GITHUB_TOKEN`

Common optional secrets:

- `CLAUDE_CODE_OAUTH_TOKEN`
- `OPENROUTER_API_KEY`
- `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`
- `RESEND_API_KEY`
- `GITHUB_WEBHOOK_SECRET`

API/frontend auth:

- `AUTH_SECRET`
- optional `AUTH_DISABLED`, `SIGNUP_DISABLED`

## Agent Profiles and Stage Assignment

Agent profiles are auto-seeded on startup (`agent/src/profiles.rs`) from `agent.backends`:

- one profile per `cli x model`
- default profile corresponds to default backend/model

Task assignment controls:

- `assigned_agent_id`: task-level profile
- `planner_agent_id`, `implementer_agent_id`, `reviewer_agent_id`: per-stage overrides

Runtime precedence:

1. stage profile ID
2. task `cli`/`model`
3. `assigned_agent_id`
4. config default backend/model

## Backend Authentication and Preflight

Preflight (`agent/src/preflight.rs`) probes configured backends on startup and logs warnings; it is non-fatal.

Expected auth mode:

- `claude`: CLI installed (`claude`), login or `CLAUDE_CODE_OAUTH_TOKEN`
- `codex`: `codex login`
- `cursor`: `cursor-agent login`
- `grok`: `grok login`
- `opencode`: `OPENROUTER_API_KEY` env

## MCP Secret Delivery Patterns

### Local development (`.env`)

Use placeholders in `config.yaml`:

```yaml
env:
  LINEAR_API_KEY: "${LINEAR_API_KEY}"
```

Then define `LINEAR_API_KEY=...` in shell or `.env`.

### Helm

Use any combination of:

- `mcpSecrets.inline`: creates key/value entries in chart-managed Secret
- `mcpSecrets.existingSecrets`: `envFrom` bulk import from existing Secrets
- `agent.extraEnv` / `agent.extraEnvFrom`: custom env wiring

Important: `config.yaml` rendered by ConfigMap should hold placeholders (`${VAR}`), not plaintext secrets.

## Deployment

### A) Local without Docker

1. Run PostgreSQL + Redis locally.
2. Set env vars (minimum):

```bash
export DATABASE_URL="postgres://karna:karna@localhost:5432/karna"
export REDIS_URL="redis://localhost:6379"
export GITHUB_TOKEN="ghp_..."
export AUTH_SECRET="$(openssl rand -hex 32)"
export CONFIG_PATH="$(pwd)/config.yaml"
export REPOS_DIR="$HOME/karna-repos"
export WORKSPACES_DIR="$HOME/karna-workspaces"
```

3. Apply migrations in order:

```bash
for f in migrations/[0-9]*.sql; do
  psql "$DATABASE_URL" -f "$f"
done
```

4. Run services:

```bash
# API
cargo run -p karna-api

# Agent (separate shell)
cargo run -p karna-agent

# Frontend (separate shell)
cd frontend
npm install
npm run setup
npm run dev
```

### B) Docker Compose

Standard:

```bash
docker compose up
```

With in-cluster mem0 stack:

```bash
docker compose --profile memory up -d mem0
```

Optional mem0 env overrides for compose:

```bash
# provider/model/base URL overrides
MEM0_LLM_PROVIDER=openai
MEM0_LLM_MODEL=openai/gpt-4o-mini
MEM0_LLM_BASE_URL=https://openrouter.ai/api/v1
MEM0_EMBEDDER_PROVIDER=openai
MEM0_EMBEDDER_MODEL=openai/text-embedding-3-small
MEM0_EMBEDDER_BASE_URL=https://openrouter.ai/api/v1

# key precedence:
# MEM0_LLM_API_KEY / MEM0_EMBEDDER_API_KEY -> MEM0_API_KEY -> MEM0_OPENROUTER_API_KEY -> OPENROUTER_API_KEY
OPENROUTER_API_KEY=sk-or-...
```

Compose stack includes `postgres`, `redis`, migration job, `api`, `agent`, `frontend`, optional `code-server`, and optional tunnel.

### C) Helm

Primary values:

- `config.repos`
- `config.backends`
- `auth.*`
- `github.*`
- `claude.*` / `openrouter.*`
- `config.memory.*` and `mem0.enabled`
- `mem0.llm.*`, `mem0.embedder.*`, and mem0 key source (`mem0.apiKey` / `mem0.existingSecret`, `mem0.openrouterApiKey` / `mem0.openrouterExistingSecret`, or global `openrouter.*`)
- `config.slack.*` and `slack.*`
- `mcpSecrets.*`
- `ingress.*`

Chart templates to inspect:

- `charts/karna/templates/configmap.yaml`
- `charts/karna/templates/secret.yaml`
- `charts/karna/templates/agent/deployment.yaml`
- `charts/karna/templates/mem0/deployment.yaml`
- `charts/karna/templates/migration-job.yaml`

## Release flow (`vX.Y.Z`)

`/.github/workflows/release.yml`:

- trigger on `v*` tag
- build/push multi-arch images (`karna-agent`, `karna-api`, `karna-frontend`, `karna-code-server`) to GHCR
- package/push OCI Helm chart
- stamp chart version/appVersion from tag

## Soft Tasks and Orchestrator Usage

### Create non-code task (`kind=doc|research|ops`)

`POST /api/tasks`

```json
{
  "title": "Write rollout notes",
  "description": "Summarize deploy risk and rollback plan",
  "kind": "doc",
  "output_target": "notification",
  "priority": "medium"
}
```

### Create orchestrator task

`POST /api/orchestrator-tasks`

```json
{
  "title": "Provider verification loop",
  "description": "Track thread replies and defer checks",
  "repo": "owner/repo",
  "priority": "high",
  "slack_channel": "C0123456789",
  "thread_ts": "1720000000.123456",
  "orchestrator": {
    "allowed_tools": ["node-watchman/*"],
    "max_turns": 12,
    "deadline": "2h",
    "max_actions_per_turn": 10,
    "max_subtasks": 5,
    "accepts_external_replies": true
  }
}
```

Chat UI calls the same endpoint with:

- `source: "chat"`
- `kind` forced to `ops` server-side
- read-only orchestrator execution surface

