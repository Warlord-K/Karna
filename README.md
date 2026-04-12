<p align="center">
  <img src="frontend/public//logo-192.png" alt="Karna" width="120" />
</p>

<h1 align="center">Karna</h1>

<p align="center">Self-hosted autonomous coding agent. Create tasks on a kanban board, an AI agent plans and implements them, opens PRs on GitHub, and notifies you via email.</p>

```
You create a task → Agent writes a plan → You review → Agent implements → Opens PR → You merge
```

## Quick Start

One command — it clones the repo, asks for your tokens, configures everything, and starts the server:

```bash
curl -fsSL https://raw.githubusercontent.com/Warlord-K/karna/main/install.sh | bash
```

You'll need:
- A **GitHub PAT** with `repo` + `workflow` scopes ([create one](https://github.com/settings/tokens))
- A **Claude Code OAuth token** (`npm install -g @anthropic-ai/claude-code && claude setup-token`)

That's it. Open [localhost:3000](http://localhost:3000) and create your first task.

<details>
<summary>Manual setup</summary>

```bash
git clone https://github.com/Warlord-K/karna.git
cd karna
cp .env.example .env
cp config.example.yaml config.yaml
```

Add your tokens to `.env`:

```bash
GITHUB_TOKEN=ghp_...
CLAUDE_CODE_OAUTH_TOKEN=...
AUTH_SECRET=$(openssl rand -hex 32)
```

Add repos to `config.yaml`:

```yaml
repos:
  - repo: you/my-app
    branch: main
```

```bash
docker compose up
```

</details>

## How It Works

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Frontend   │────▶│   Postgres   │◀────│  Rust Agent  │
│  Next.js     │     │   + Redis    │     │  polls every │
│  :3000       │     │              │     │  30 seconds  │
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                  │
                                       ┌──────────┼──────────┐
                                       ▼          ▼          ▼
                                  Claude Code   git/gh    Resend
                                  or Codex      (PRs)     (email)
```

**Task lifecycle:**

| Status | What happens |
|--------|-------------|
| **Todo** | You create a task with title, description, repo, priority |
| **Planning** | Agent picks it up, explores the codebase, writes a plan |
| **Plan Review** | You read the plan, approve or request changes |
| **In Progress** | Agent implements the approved plan |
| **Review** | Agent opens a PR on GitHub |
| **Done** | You merge the PR |

You can send feedback at any stage — the agent re-plans or updates the PR.

### Multi-Repo Tasks

Tasks can span multiple repositories. Create a task without selecting a specific repo and the agent will:

1. Explore all configured repos and generate a plan with per-repo subtasks
2. You approve the plan — subtasks are created automatically
3. Each subtask goes through the normal lifecycle independently
4. Parent task auto-completes when all subtasks finish

### CLI Backends

Tasks can use either **Claude Code** or **OpenAI Codex** as the AI backend. Pick the CLI and model when creating a task.

| Backend | Models | MCP Support | Auth |
|---------|--------|-------------|------|
| `claude` | opus, sonnet, haiku | Yes | `claude setup-token` → set `CLAUDE_CODE_OAUTH_TOKEN` in `.env` |
| `codex` | gpt-5.4, gpt-5.4-mini, gpt-5.3-codex | No | `codex login` → mounts `~/.codex` automatically |

## Services

| Port | Service | Purpose |
|------|---------|---------|
| [localhost:3000](http://localhost:3000) | Frontend | Kanban board + task management |
| [localhost:8080](http://localhost:8080) | Agent API | Webhooks + health check |
| [localhost:8443](http://localhost:8443) | code-server | Browser IDE — watch the agent work live |

## Scaling

Run multiple agents in parallel:

```bash
docker compose up --scale agent=3
```

Each agent competes for tasks via Redis distributed locks. No double-work. Dead workers auto-release after 30 minutes.

## Repo Onboarding

When repos are added, the agent automatically profiles them — detecting language, framework, test commands, build tools, and key directories. These profiles power smarter planning for multi-repo tasks. Manage repos from the UI or `config.yaml`.

## Schedules

Automated agent runs on a cron or one-shot basis. The agent explores your repos and optionally creates tasks on the board.

Two built-in schedules ship disabled by default:

| Schedule | Cron | What it does |
|----------|------|-------------|
| **Bug Hunter** | Every 6 hours | Scans for bugs, security issues, race conditions |
| **Feature Scout** | Monday 9am | Finds TODOs, dead code, performance bottlenecks |

Create custom schedules from the UI or `config.yaml`. Each run gets its own logs and can auto-create tasks with configurable limits.

## Skills

9 built-in skills ship in `skills/` (auto-loaded):

| Skill | What it does |
|-------|-------------|
| `test` | Auto-detect test framework, run tests |
| `lint` | Auto-detect linter, run with auto-fix |
| `typecheck` | Run static type checking |
| `commit` | Conventional commit format |
| `review` | Self-review checklist before PR |
| `build` | Auto-detect build system, verify build |
| `migrate` | Database migration guidance |
| `security` | Security scan + dependency audit |
| `add-repo` | Onboard a repository — explore and generate profile |

**Add custom skills** — drop a markdown file in `skills/`:

```markdown
---
description: Deploy to staging
command: gh workflow run deploy.yml -f environment=staging
phase: implement
---
Run this after all tests pass to deploy the changes.
```

Repos can also ship their own `skills/` directory — auto-discovered when the agent clones them.

## MCP Servers

5 MCP servers are enabled by default (no API keys needed):

| Server | Purpose |
|--------|---------|
| **fetch** | Read any URL as markdown |
| **context7** | Up-to-date library documentation |
| **memory** | Persistent knowledge graph across tasks |
| **sequential-thinking** | Structured reasoning |
| **github** | Full GitHub API (uses your GITHUB_TOKEN) |

Add more in `config.yaml` — Sentry, Linear, Slack, Postgres, Supabase, Notion, Brave Search, and Playwright are preconfigured and just need API keys.

Repos can provide `.mcp.json` at their root — auto-discovered and merged at runtime.

## Agent Instructions

Give the agent persistent context across all tasks with a markdown instructions file:

```yaml
agent:
  instructions: instructions.md
```

Use this for agent identity, a map of your repos, cross-repo conventions, and architectural context. See `instructions.example.md` for the recommended format.

## Self-Iteration

Karna can manage its own repository — modifying config, skills, instructions, and its own code:

```yaml
repos:
  - repo: user/karna-fork
    branch: main
    self: true
```

The agent implements changes and opens PRs like any other task. Config and skill changes hot-reload instantly. Code changes trigger an automatic rebuild with graceful drain (no mid-task interruption).

## GitHub Webhooks

Connect GitHub webhooks so the agent picks up PR reviews and comments in real-time.

**Setup:** Make the agent reachable from GitHub (Cloudflare Tunnel, public IP, or ngrok), then set `AGENT_WEBHOOK_URL` or `TUNNEL_AGENT_HOSTNAME` in `.env`. Webhooks are auto-registered on repos during onboarding. Optionally set `GITHUB_WEBHOOK_SECRET` for HMAC-SHA256 signature verification.

| You do this on GitHub | Agent does this |
|----------------------|----------------|
| Request changes on a PR | Reads your review, pushes fixes |
| Comment on a PR | Addresses your feedback |
| Approve + merge a PR | Marks the task as done |

**Without webhooks:** Post feedback through the Activity tab in the UI. The agent also gathers PR comments via `gh pr view` when it starts working on feedback.

## Commit Signing

Sign agent commits with an SSH key so they show as "Verified" on GitHub:

```bash
mkdir -p signing
cp ~/.ssh/id_ed25519 signing/signing_key
```

Add the **public** key to [GitHub SSH settings](https://github.com/settings/ssh/new) as a "Signing Key". The agent auto-detects keys in `signing/` at startup. If empty, signing is skipped.

## Public Access (Cloudflare Tunnel)

Expose your instance to the internet without port forwarding:

**Token-based** (routes managed in CF dashboard):
```bash
CLOUDFLARE_TUNNEL_TOKEN=your-token
docker compose --profile tunnel up
```

**Credentials-based** (routes defined locally):
```bash
cloudflared tunnel create karna
cloudflared tunnel route dns karna app.yourdomain.com
cloudflared tunnel route dns karna code.yourdomain.com

# Add to .env:
CLOUDFLARE_TUNNEL_ID=...
CLOUDFLARE_TUNNEL_CREDENTIALS=...   # base64 of ~/.cloudflared/<TUNNEL_ID>.json
TUNNEL_FRONTEND_HOSTNAME=app.yourdomain.com
TUNNEL_CODE_SERVER_HOSTNAME=code.yourdomain.com
```

## CLI

```bash
./karna start      # Start all services + background auto-updater
./karna stop       # Graceful shutdown (10min drain)
./karna restart    # Stop + start
./karna update     # Manual update check
./karna status     # Service health + updater state
./karna setup      # Validate config, test connections
./karna logs       # Tail logs (optionally: ./karna logs agent)
```

## Configuration Reference

### `config.yaml`

```yaml
repos:
  - repo: you/backend
    branch: main
  - repo: you/frontend
    branch: main

agent:
  max_turns: 100              # Max CLI turns per invocation
  poll_interval_secs: 30      # Task polling frequency
  max_concurrent_tasks: 1     # Per worker (increase with --scale)
  instructions: instructions.md  # Optional system prompt file

  backends:
    claude:
      models: [opus, sonnet, haiku]
      default_model: sonnet
    codex:
      models: [gpt-5.4, gpt-5.4-mini, gpt-5.3-codex]
      default_model: gpt-5.4

notifications:
  email: you@example.com
```

### Environment Variables

| Variable | Required | Purpose |
|----------|----------|---------|
| `GITHUB_TOKEN` | Yes | Git operations, PRs ([scopes: repo, workflow](https://github.com/settings/tokens)) |
| `CLAUDE_CODE_OAUTH_TOKEN` | Yes (Claude) | Claude Code CLI auth |
| `AUTH_SECRET` | Yes | Session encryption (`openssl rand -hex 32`) |
| `AUTH_DISABLED` | No | Set `true` to skip auth (single-user mode) |
| `SIGNUP_DISABLED` | No | Default: signups disabled. Set `false` to allow registration |
| `RESEND_API_KEY` | No | Email notifications via [Resend](https://resend.com) |
| `GIT_AUTHOR_NAME` | No | Default: `Karna Agent` |
| `GIT_AUTHOR_EMAIL` | No | Default: `agent@karna.dev` |
| `CODE_SERVER_PASSWORD` | No | Default: `changeme` |
| `AGENT_WEBHOOK_URL` | No | Webhook URL for GitHub (e.g. ngrok URL) |
| `GITHUB_WEBHOOK_SECRET` | No | HMAC-SHA256 verification (`openssl rand -hex 32`) |
| `CLOUDFLARE_TUNNEL_TOKEN` | No | CF tunnel (token mode) |

## Backup & Restore

### Backup (on old machine)

```bash
cd ~/karna

# 1. Database (tasks, users, logs, schedules, repo profiles)
docker compose exec postgres pg_dump -U karna karna > backup.sql

# 2. Bundle config + secrets + signing keys
tar czf karna-config.tar.gz .env config.yaml signing/ instructions.md skills/ 2>/dev/null
```

### Restore (on new machine)

```bash
# 1. Install Karna
curl -fsSL https://raw.githubusercontent.com/Warlord-K/karna/main/install.sh | bash
# (or: git clone https://github.com/Warlord-K/karna.git ~/karna)

# 2. Stop services, restore config
cd ~/karna
docker compose down
tar xzf /path/to/karna-config.tar.gz

# 3. Start Postgres, wait for migrations, restore data
docker compose up -d postgres
sleep 5
docker compose exec -T postgres psql -U karna karna < backup.sql

# 4. Start everything
docker compose up -d
```

### What's backed up where

| Data | Backed up by | Notes |
|------|-------------|-------|
| Tasks, logs, users, schedules | `backup.sql` | All stateful data |
| Tokens + secrets | `.env` | GITHUB_TOKEN, CLAUDE_CODE_OAUTH_TOKEN, AUTH_SECRET |
| Repos + agent config | `config.yaml` | Tracked in git if using self-iteration |
| Signing keys | `signing/` | Only if using commit signing |
| Agent instructions | `instructions.md` | Only if configured |
| Custom skills | `skills/` | Built-in skills are in git, custom ones need backup |

### Migrate to external Postgres

```bash
# Dump from local
docker compose exec postgres pg_dump -U karna karna > backup.sql

# Restore to remote (Supabase, Neon, RDS, etc.)
psql "$REMOTE_DATABASE_URL" < backup.sql
```

## Development

```bash
# Infrastructure only
docker compose up -d postgres redis

# Frontend (hot reload)
cd frontend && npm install && npm run setup && npm run dev

# Agent
cd agent
export DATABASE_URL="postgres://karna:karna@localhost:5432/karna"
export REDIS_URL="redis://localhost:6379"
export CONFIG_PATH="../config.yaml"
export REPOS_DIR="$HOME/karna-repos"
export WORKSPACES_DIR="$HOME/karna-workspaces"
cargo run
```

CI runs on all PRs: `cargo check` + `cargo clippy` for the agent, `npm run build` for the frontend, and Docker build verification.

## Architecture

```
docker-compose.yml
├── postgres:16       — Auth sessions + tasks + logs
├── redis:7           — Task queue + distributed locks
├── agent (Rust)      — Polls DB, invokes Claude Code or Codex CLI, git/gh operations
├── frontend (Next.js)— Kanban board, Auth.js with credentials auth
├── code-server       — Browser IDE to watch agent edit files
├── tunnel            — Optional Cloudflare Tunnel for public access
└── autoheal          — Auto-restarts unhealthy containers
```

**Tech stack:** Rust (Tokio, Axum, sqlx, redis) · Next.js 15, React 19, Auth.js v5, Tailwind, dnd-kit, Framer Motion · PostgreSQL 16 · Redis 7 · Claude Code CLI + OpenAI Codex CLI

## License

MIT
