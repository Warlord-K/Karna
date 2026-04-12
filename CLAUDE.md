# Karna — Self-Hosted Autonomous Coding Agent

Create tasks on a kanban board, an AI agent plans and implements them, opens PRs on GitHub, and notifies you via email. You review plans and PRs, provide feedback, and the agent iterates.

## Architecture

```
docker-compose.yml
├── postgres:16     — Auth.js sessions + agent_tasks + agent_logs
├── redis:7         — Task queue + distributed locks
├── agent (Rust)    — Polls DB, invokes Claude Code or Codex CLI, git/gh operations
├── frontend (Next.js) — Kanban board UI, Auth.js + email/password credentials
├── code-server     — Browser IDE with dev tooling (git, gh, claude, codex) + config-driven extensions
├── tunnel          — Optional Cloudflare Tunnel for public access
└── autoheal        — Auto-restarts unhealthy containers
```

## Tech Stack

### Frontend (`frontend/`)
- **Framework**: Next.js 15 (App Router), React 19, TypeScript
- **Auth**: Auth.js v5 + email/password credentials + @auth/pg-adapter (Postgres sessions)
- **Styling**: Tailwind CSS 3 + shadcn/ui
- **DnD**: @dnd-kit/core + @dnd-kit/sortable
- **Animations**: Framer Motion
- **State**: Client-side fetch + 5s polling (no WebSocket/Realtime)

### Agent Backend (`agent/`)
- **Language**: Rust (2024 edition)
- **Runtime**: Tokio
- **HTTP**: Axum (health + webhooks)
- **DB**: sqlx (Postgres)
- **Queue**: Redis (distributed locks with NX + EX)
- **AI**: Pluggable CLI backends — Claude Code or OpenAI Codex (spawned as subprocess, configured via `agent.cli`)
- **Git**: git + gh CLI

### Infrastructure
- **DB**: PostgreSQL 16
- **Cache/Queue**: Redis 7
- **IDE**: code-server (VS Code in browser, custom build with git/gh/claude/codex + configurable extensions/settings via config.yaml)
- **Deployment**: Docker Compose (scales with `--scale agent=N`)

## Project Structure

```
karna/
├── docker-compose.yml
├── install.sh                   # One-line installer (curl | bash)
├── karna                        # CLI wrapper: start/stop/update/setup (replaces raw docker compose)
├── config.example.yaml          # Config template (tracked in git)
├── config.yaml                  # User configuration — gitignored (cp config.example.yaml config.yaml)
├── instructions.example.md      # Sample agent instructions file (identity, repo map, conventions)
├── .env.example                 # Secrets (API keys, OAuth)
├── .github/workflows/ci.yml    # CI: cargo check/clippy, next.js build, Docker build on PRs
├── migrations/
│   ├── 001_initial.sql          # Auth.js tables + agent_tasks + agent_logs
│   ├── 002_add_password.sql     # Password field for users
│   ├── 003_subtasks.sql         # Subtask support (parent_task_id, nullable repo)
│   ├── 004_cli_model.sql        # Per-task CLI backend + model selection
│   ├── 005_task_number.sql      # Task numbering
│   ├── 006_log_type_tool.sql    # Log type/tool tracking
│   ├── 007_cost_usd.sql         # Cost tracking in USD
│   ├── 008_comment_log_type.sql # Comment log type
│   ├── 009_schedules.sql        # schedules + scheduled_runs + scheduled_run_logs
│   ├── 010_repo_profiles.sql    # Repo profiles for auto-discovery + smart planning
│   └── 011_cancelled_status.sql # Cancelled task status
├── agent/
│   ├── Cargo.toml
│   ├── Dockerfile
│   └── src/
│       ├── main.rs              # Entry point: poll loop, graceful shutdown, config hot-reload
│       ├── cli.rs               # Common CliOptions/CliResult + dispatch (claude|codex)
│       ├── scheduler.rs         # Schedule evaluation, execution, task creation from cron/one-shot runs
│       ├── onboarding.rs        # Repo profile auto-discovery + smart planning support
│       ├── claude/mod.rs        # Claude Code CLI runner
│       ├── codex/mod.rs         # OpenAI Codex CLI runner
│       └── updater.rs           # Self-repo change detection + classification
├── frontend/
│   ├── Dockerfile
│   ├── package.json
│   ├── auth.ts                  # Auth.js config (credentials provider + pg adapter)
│   ├── middleware.ts             # Route protection
│   ├── app/
│   │   ├── layout.tsx           # Root layout with SessionProvider
│   │   ├── globals.css          # Tailwind + CSS variables (dark theme)
│   │   ├── page.tsx             # Kanban board (6 columns, DnD, polling)
│   │   ├── login/page.tsx       # Email/password login + signup
│   │   └── api/
│   │       ├── auth/[...nextauth]/route.ts  # Auth.js handler
│   │       ├── tasks/
│   │       │   ├── route.ts     # GET (list), POST (create — repo optional)
│   │       │   └── [id]/
│   │       │       ├── route.ts      # PATCH (update), DELETE
│   │       │       ├── logs/route.ts # GET logs
│   │       │       └── subtasks/route.ts # GET (list), POST (create from plan)
│   │       ├── schedules/
│   │       │   ├── route.ts     # GET (list), POST (create schedule)
│   │       │   └── [id]/
│   │       │       ├── route.ts      # GET, PATCH (update), DELETE
│   │       │       ├── trigger/route.ts # POST (manual trigger via Redis)
│   │       │       └── runs/
│   │       │           ├── route.ts  # GET (list runs)
│   │       │           └── [runId]/logs/route.ts # GET (run logs)
│   │       └── repos/
│   │           ├── route.ts     # GET (list), POST (add repo)
│   │           └── [id]/
│   │               ├── route.ts      # DELETE
│   │               └── onboard/route.ts # POST (trigger re-onboarding)
│   ├── lib/
│   │   ├── db.ts                # pg Pool
│   │   ├── agent-tasks.ts       # Types + API client helpers
│   │   ├── schedules.ts         # Schedule types + API client + cron helpers
│   │   ├── repos.ts             # Repo profile types + API client helpers
│   │   └── utils.ts             # cn() utility
│   └── components/agent/
│       ├── agent-column.tsx     # Kanban column with droppable zone
│       ├── task-card.tsx        # Task card (priority, status, repo badge)
│       ├── create-task-dialog.tsx # Create task form
│       ├── task-detail-modal.tsx  # Detail view (tabs: Details, Plan, Activity w/ inline comments)
│       ├── schedules-page.tsx   # Schedules list page (CRUD, toggle, trigger)
│       ├── schedule-card.tsx    # Schedule card (name, cron, last run, toggle)
│       ├── create-schedule-dialog.tsx # Create schedule form (cron/one-shot, repos, skills, MCP)
│       ├── schedule-detail-modal.tsx  # Schedule detail (tabs: Runs, Details; run summary + logs)
│       ├── repos-page.tsx       # Repos list page (add, onboard, delete)
│       ├── repo-card.tsx        # Repo card (status, language, branch, actions)
│       ├── add-repo-dialog.tsx  # Add repo form (owner/repo, branch)
│       └── repo-detail-modal.tsx # Repo detail (profile info, commands, directories, summary)
├── code-server/
│   ├── Dockerfile               # Custom code-server with dev tooling (git, gh, node, claude, codex)
│   └── setup.sh                 # Config-driven extension + settings installer (runs via entrypoint.d)
└── .env.example                 # Copy to .env, fill in secrets
```

## Database Schema

### Auth.js tables (managed by @auth/pg-adapter)
- `users` — User profiles (email/password)
- `accounts` — Account links
- `sessions` — Active sessions
- `verification_tokens` — Email verification

### Agent tables
- `agent_tasks` — Task definitions + state + artifacts (plan, PR, feedback)
  - `parent_task_id` (UUID, nullable) — FK to parent agent_task for subtask hierarchy
  - `repo` (TEXT, nullable) — NULL for multi-repo parent tasks; subtasks carry the repo
  - `cli` (TEXT, nullable) — CLI backend ("claude", "codex"); NULL = config default
  - `model` (TEXT, nullable) — Model name ("sonnet", "gpt-5.4"); NULL = backend default
- `agent_logs` — Append-only agent activity log per task (includes user comments with `log_type = 'comment'`)

### Schedule tables
- `schedules` — Schedule definitions (cron or one-shot), prompt, repos, skills, MCP servers, task creation config
  - `cron_expression` (TEXT, nullable) — 5-field cron for recurring schedules
  - `run_at` (TIMESTAMPTZ, nullable) — one-shot execution time (auto-disables after completion)
  - `max_open_tasks` (INTEGER) — limit on concurrent open tasks with matching prefix
  - `task_prefix` (TEXT, nullable) — prefix for created task titles (e.g. "BUG", "FEA")
  - `skills` (TEXT[]) — skill names to inject into the prompt
  - `mcp_servers` (TEXT[]) — MCP server names for the run
- `scheduled_runs` — One record per schedule execution (status, summary_markdown, tasks_created, cost_usd)
- `scheduled_run_logs` — Append-only logs per run (mirrors agent_logs pattern)

### Repo profile tables
- `repo_profiles` — Auto-discovered repository profiles for smart multi-repo planning
  - `repo` (TEXT, UNIQUE) — "owner/repo" format
  - `branch` (TEXT) — default branch to track
  - `status` (TEXT) — pending → onboarding → ready (or failed/stale)
  - `summary` (TEXT) — human-readable markdown profile from CLI exploration
  - `profile_json` (JSONB) — structured data: language, framework, commands, directories, CI
  - `last_commit_sha` (TEXT) — tracks staleness (HEAD changed since last onboard)
  - `cost_usd` (DOUBLE PRECISION) — accumulated onboarding cost

## Task State Machine

```
Single-repo tasks:
TODO → PLANNING → PLAN_REVIEW → IN_PROGRESS → REVIEW → DONE
                  ↑ (reject)                    ↑ (changes)
                  └────────────┘                └───────────┘
Any state → FAILED → TODO (retry)
Any non-terminal state → CANCELLED (user dismisses)

Multi-repo (parent) tasks:
TODO → PLANNING → PLAN_REVIEW → [approve creates subtasks] → IN_PROGRESS (waiting) → DONE
                                                                  ↑ auto when all subtasks done/cancelled

Subtasks (children):
TODO → PLANNING → PLAN_REVIEW → IN_PROGRESS → REVIEW → DONE
(same as single-repo, each subtask targets one repo)
```

| Status | Who triggers | What happens |
|--------|-------------|-------------|
| todo | User creates task | Queued for agent |
| planning | Agent picks up | Claude Code explores + generates plan |
| plan_review | Agent finishes plan | User reviews in Plan tab |
| in_progress | User approves plan | Claude Code implements |
| review | Agent opens PR | User reviews on GitHub or in app |
| done | User merges PR | Task complete |
| failed | Error during agent work | User can retry |
| cancelled | User cancels task | Task dismissed, shown in Done column |

## Subtasks (Multi-Repo Tasks)

Tasks can span multiple repositories. When a task is created without a specific repo (repo = NULL), the agent treats it as a multi-repo parent task:

1. **Planning**: Agent explores all configured repos and generates a plan with a `<!-- subtasks -->` JSON block
2. **Plan Approval**: Frontend detects the subtask block; "Approve Plan" calls `POST /api/tasks/{id}/subtasks` which parses the plan and creates child tasks
3. **Execution**: Parent moves to `in_progress` (waiting). Each subtask goes through the normal single-repo lifecycle independently
4. **Completion**: A DB trigger auto-completes the parent when all subtasks reach `done`

**Plan subtask format** (embedded in plan_content markdown):
```
<!-- subtasks
[
  {"title": "Update API models", "repo": "owner/backend", "description": "Add new fields to..."},
  {"title": "Add UI components", "repo": "owner/frontend", "description": "Create form for..."}
]
subtasks -->
```

**Key behaviors:**
- Parent tasks with subtasks are excluded from `next_actionable_task()` and `has_active_task()` — only subtasks are worked on
- Subtasks are hidden from the Kanban board (nested under parent via `nestSubtasks()`)
- The TaskCard shows a progress bar for parent tasks (X/N subtasks complete)
- The TaskDetailModal shows a "Subtasks" tab with per-subtask status, repo, and PR links
- Deleting a parent cascades to all subtasks (ON DELETE CASCADE)

## API Routes

| Method | Path | Purpose |
|--------|------|---------|
| GET | /api/tasks | List all tasks for current user |
| POST | /api/tasks | Create new task (repo optional for multi-repo) |
| PATCH | /api/tasks/[id] | Update task fields |
| DELETE | /api/tasks/[id] | Delete task |
| GET | /api/tasks/[id]/logs | Get agent logs for task |
| POST | /api/tasks/[id]/comments | Post a comment (creates log entry + sets feedback for agent) |
| GET | /api/tasks/[id]/subtasks | List subtasks for a parent task |
| POST | /api/tasks/[id]/subtasks | Parse plan & create subtasks (plan approval) |
| GET/POST | /api/auth/* | Auth.js handlers |
| GET | /api/schedules | List all schedules for current user (with last run) |
| POST | /api/schedules | Create new schedule (cron or one-shot) |
| GET | /api/schedules/[id] | Get single schedule |
| PATCH | /api/schedules/[id] | Update schedule fields (name, prompt, enabled, etc.) |
| DELETE | /api/schedules/[id] | Delete schedule (cascades to runs + logs) |
| POST | /api/schedules/[id]/trigger | Manual trigger (sets Redis key for immediate pickup) |
| GET | /api/schedules/[id]/runs | List runs for a schedule |
| GET | /api/schedules/[id]/runs/[runId]/logs | Get logs for a specific run |
| GET | /api/repos | List all repo profiles for current user |
| POST | /api/repos | Add new repo (triggers onboarding) |
| DELETE | /api/repos/[id] | Delete repo profile |
| POST | /api/repos/[id]/onboard | Trigger re-onboarding for a repo |

## Frontend Components

| Component | Purpose |
|-----------|---------|
| `agent-column.tsx` | Kanban column with droppable zone, task count |
| `task-card.tsx` | Draggable card: priority dot, status indicator, repo badge, PR link |
| `create-task-dialog.tsx` | Modal: title, description, repo selector, priority |
| `task-detail-modal.tsx` | 3-tab modal: Details (edit), Plan (approve/reject), Activity (logs + inline comments) |
| `schedules-page.tsx` | Schedules list with CRUD, enable/disable, trigger, 10s polling |
| `schedule-card.tsx` | Schedule card: name, cron/one-shot, last run status, pause/play/trigger |
| `create-schedule-dialog.tsx` | Modal: name, prompt, cron presets, repos, skills, MCP servers, prefix |
| `schedule-detail-modal.tsx` | 2-tab modal: Runs (list + detail with markdown summary + logs), Details (config) |
| `repos-page.tsx` | Repos list with add, onboard, delete, 5s polling |
| `repo-card.tsx` | Repo card: name, status badge, language/framework, branch, actions |
| `add-repo-dialog.tsx` | Modal: owner/repo input, branch |
| `repo-detail-modal.tsx` | Repo detail: profile info grid, commands, directories, summary markdown |

## Development

```bash
# First time
cp .env.example .env        # fill in API keys and OAuth creds
cp config.example.yaml config.yaml  # add your repos
./karna setup               # validate config, test tokens
./karna start               # start all services + auto-updater

# Or without auto-update
docker compose up

# Scale agents
docker compose up --scale agent=3
```

## Self-Iteration

Karna can manage its own repo — modifying config, skills, instructions, and even its own code. Add the self-repo to config.yaml:

```yaml
repos:
  - repo: user/karna-fork
    branch: main
    self: true     # Enables self-iteration
```

### How it works

1. **Task targeting self-repo**: Create a task like "add a deployment skill" or "update instructions with new repo conventions"
2. **Agent implements + opens PR**: Normal task lifecycle — plan, implement, PR
3. **User merges PR**: Changes land on main
4. **Auto-update detects changes**: The `./karna` wrapper polls git every 5 minutes

### What the agent can self-modify

`config.yaml` is gitignored (user-specific). `config.example.yaml` is tracked as a template. The agent can directly:

- **Add skills**: create `skills/deploy.md` with frontmatter + prompt
- **Add MCP servers**: append to `mcp_servers` in `config.yaml`
- **Update instructions**: edit `instructions.md` as it learns about repos
- **Modify backends**: change default models, add new backends
- **Improve its own code**: modify `agent/src/**` or `frontend/**`
- **Add CI checks**: update `.github/workflows/ci.yml`

### Change categories

| Changed files | Action | Downtime |
|---------------|--------|----------|
| `skills/*.md`, `instructions.md`, `config.yaml` | Hot-reload (agent re-reads on next poll) | None |
| `agent/src/**`, `agent/Cargo*`, `agent/Dockerfile` | Rebuild + restart agent container | ~seconds |
| `frontend/**` | Rebuild + restart frontend container | ~seconds |
| `docker-compose.yml`, `migrations/**` | Rebuild all | ~seconds |

### Graceful drain

When a rebuild is needed, the agent:
1. Finishes the current task (no mid-task interruption)
2. Releases all Redis locks
3. Exits with code 42
4. The wrapper script detects exit 42, pulls latest, rebuilds, restarts

The `stop_grace_period: 10m` in docker-compose gives long-running tasks time to complete.

### Agent-side detection

The agent also monitors the self-repo from inside the container (`updater.rs`):
- Fetches remote on each poll cycle
- Compares local HEAD vs remote
- Classifies changed files
- For code changes: sets shutdown flag, drains, exits with code 42
- For config changes: no action (hot-reload handles it)

### CI

GitHub Actions (`.github/workflows/ci.yml`) runs on all PRs:
- `cargo check` + `cargo clippy` for agent
- `npm run build` for frontend
- Docker build verification

This ensures the agent can't merge a PR that breaks the build.

### karna CLI

```bash
./karna start     # Start all + auto-updater
./karna stop      # Graceful shutdown
./karna update    # Manual update check
./karna status    # Service status + updater state
./karna setup     # Validate config
./karna logs      # Tail logs
```

## Schedules (Automated Runs)

DB-backed schedules that run prompts on a cron or one-shot basis, explore repos via CLI, and optionally create tasks on the kanban board.

**Execution flow:**
1. User creates schedule via frontend (or agent creates one for itself later)
2. Agent poll loop calls `scheduler::check_schedules()` every iteration
3. For each enabled schedule: evaluate cron expression against last run time (or check `run_at` for one-shot)
4. If due (or manually triggered via Redis key `schedule_trigger:{id}`): acquire Redis lock, check `max_open_tasks`, create `scheduled_runs` record
5. Clone/fetch repos, build prompt with skills/MCP, invoke CLI with read-only tools (`Read,Glob,Grep,Bash`)
6. Parse output for `<!-- tasks [...] tasks -->` block, create tasks on the board
7. Update run record with summary markdown, task IDs, cost, status
8. One-shot schedules auto-disable after completion

**Key files:**
- `agent/src/scheduler.rs` — Schedule evaluation, execution, task creation
- `agent/templates/schedule_prompt.txt` — Prompt template for schedule runs
- `frontend/lib/schedules.ts` — Types + API client + cron display helpers
- `frontend/components/agent/schedules-page.tsx` — Main schedules UI
- `migrations/009_schedules.sql` — DB schema

**Redis keys:**
- `schedule_lock:{schedule_id}` — Prevents duplicate execution across workers (30min TTL)
- `schedule_trigger:{schedule_id}` — Manual trigger from frontend "Run Now" button (5min TTL)

## GitHub Webhooks (PR Feedback)

The agent receives PR feedback from GitHub via webhooks — it does **not** poll GitHub for reviews.

**Endpoint:** `POST /webhooks/github` on the agent's Axum server (`:8080`)

**Handled events:**

| GitHub Event | Action | Agent Behavior |
|-------------|--------|----------------|
| `pull_request_review` | `changes_requested` | Sets `task.feedback`, transitions `review → in_progress` |
| `pull_request_review` | `approved` | Logs only (user must merge manually) |
| `pull_request` | `closed` + merged | Transitions task to `done`, sends notification |
| `issue_comment` | `created` (on PR) | Appends comment to `task.feedback` |

**Branch filtering:** Only processes branches starting with `kar-` (line 149 of `api/mod.rs`), matching the `kar-{number}/{slug}` format generated by `AgentTask::agent_branch_name()`. All other webhook events are ignored.

**Task lookup:** Uses `db.find_task_by_branch(branch)` to match the webhook to a task.

**Feedback flow:**
1. Webhook sets `task.feedback` + transitions status
2. Agent poll loop detects non-empty feedback via `tasks_with_pending_feedback()` (checked before claiming new work)
3. For `review` tasks: calls `implementer::apply_feedback()` which also runs `gh pr view --json reviews,comments` to gather all PR comments
4. Agent clears feedback after work (race-safe: only clears if no new feedback arrived during execution)

**Three feedback paths (in order of priority):**
1. **GitHub webhook** — real-time PR reviews and comments (requires webhook setup)
2. **Frontend Activity tab** — user posts comment via `POST /api/tasks/{id}/comments`, sets feedback + transitions state
3. **PR comment gathering** — `gh pr view` pulls all comments when agent starts working on feedback (catches anything webhooks missed)

**Without webhooks:** Frontend Activity tab comments are the only way to send feedback. The agent still gathers PR comments via `gh pr view` when it starts working, so inline code review comments are picked up — just not in real-time.

**Port exposure:** Agent API is on host port `${AGENT_API_PORT:-8080}` (docker-compose). For public access, configure `TUNNEL_AGENT_HOSTNAME` in `.env` (credentials-based tunnel) or add a route in the CF dashboard (token-based tunnel).

**Auto-registration:** Webhooks are automatically registered on repos during onboarding when a public URL is available. The agent derives `webhook_url` from: `AGENT_WEBHOOK_URL` env → `TUNNEL_AGENT_HOSTNAME` env (prefixed with `https://`) → None. If available, `onboard_repo()` calls `github::ensure_repo_webhook()` after profiling completes. Idempotent — checks existing hooks first. Requires `admin:repo_hook` scope on `GITHUB_TOKEN`; logs a warning and continues if missing.

**Signature verification:** When `GITHUB_WEBHOOK_SECRET` is set, the handler verifies `X-Hub-Signature-256` using HMAC-SHA256. If no secret is configured, all payloads are accepted (verification disabled). The same secret is passed to GitHub when auto-registering webhooks during onboarding.

**Key files:**
- `agent/src/api/mod.rs` — Webhook handler + HMAC-SHA256 verification
- `agent/src/agent/mod.rs` — Poll loop feedback detection
- `agent/src/agent/implementer.rs` — Feedback application
- `agent/src/git/github.rs` — PR comment gathering + `ensure_repo_webhook()` (auto-registration)
- `agent/src/onboarding.rs` — Calls webhook registration after repo profile is stored
- `agent/src/config.rs` — `webhook_url` derivation (AGENT_WEBHOOK_URL → TUNNEL_AGENT_HOSTNAME → None)
- `frontend/app/api/tasks/[id]/comments/route.ts` — Frontend comment → feedback path

## Redis Queue Protocol

```
# Claim task (atomic, one worker wins)
SET task_lock:{task_id} {worker_id} NX EX 1800

# Heartbeat (extend while working)
EXPIRE task_lock:{task_id} 1800

# Release (on completion or failure)
DEL task_lock:{task_id}
```

## Environment Variables

All secrets live in `.env` (gitignored). User config lives in `config.yaml` (gitignored). Template lives in `config.example.yaml` (tracked).

| Variable | Required | Purpose |
|----------|----------|---------|
| CLAUDE_CODE_OAUTH_TOKEN | Yes (cli: claude) | Claude Code CLI (OAuth token from `claude setup-token`) |
| GITHUB_TOKEN | Yes | Git operations, PR creation |
| AUTH_SECRET | Yes | Auth.js session encryption (generate with `openssl rand -hex 32`) |
| DATABASE_URL | Auto | Set by docker-compose |
| REDIS_URL | Auto | Set by docker-compose |
| RESEND_API_KEY | No | Email notifications |
| GIT_SIGNING_KEY | No | Override: path to SSH private key inside container |
| GIT_ALLOWED_SIGNERS | No | Override: path to allowed_signers file inside container |
| AGENT_API_PORT | No (default 8080) | Host port for agent API (webhooks, health) |
| TUNNEL_AGENT_HOSTNAME | No | Hostname for agent API (CF tunnel); also webhook URL fallback |
| AGENT_WEBHOOK_URL | No | Full URL override for webhook registration (e.g. ngrok URL) |
| GITHUB_WEBHOOK_SECRET | No | HMAC-SHA256 secret for webhook signature verification |

## Code Server (Browser IDE)

Custom-built code-server with full dev tooling: git, gh, node 22, Claude Code CLI, Codex CLI. Configurable via `code_server:` section in config.yaml.

**Config fields (all optional — defaults used if section omitted):**

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `theme` | string | `"Default Dark Modern"` | VS Code color theme |
| `extensions` | list | See below | Extensions to install on startup |
| `settings` | dict | See below | VS Code settings (overrides base defaults) |

**Default extensions:** `GitHub.github-vscode-theme`, `ms-python.python`, `rust-lang.rust-analyzer`, `dbaeumer.vscode-eslint`, `esbenp.prettier-vscode`, `bradlc.vscode-tailwindcss`, `anthropic.claude-code`

**Base settings (always applied, overridable via `settings:`):**
- `security.workspace.trust.enabled: false`
- `editor.fontSize: 14`, `editor.tabSize: 2`, `editor.formatOnSave: true`
- `editor.minimap.enabled: false`, `files.autoSave: afterDelay`
- `telemetry.telemetryLevel: off`, `git.autofetch: true`

**How it works:** `code-server/setup.sh` runs via the stock `/entrypoint.d/` mechanism before code-server starts. It reads config.yaml with `yq`, installs extensions via `code-server --install-extension`, and generates `settings.json` by merging base defaults with custom settings. Extensions are persisted in a named Docker volume (`code-server-extensions`) so they survive restarts without re-downloading.

**Volumes:**
- `workspace:/workspace` — shared with agent (see agent work in real-time)
- `./config.yaml:/etc/karna/config.yaml:ro` — config for setup script
- `code-server-extensions` — persisted extensions cache

**Environment:** `GITHUB_TOKEN` and `GH_TOKEN` are passed through for git/gh operations. `.env` is loaded via `env_file` for Claude/Codex auth tokens.

## Commit Signing (Optional, Auto-Detected)

Drop an SSH private key into the `./signing/` directory and all agent commits are automatically signed. No config changes needed.

```bash
# One-time setup
mkdir -p signing
cp ~/.ssh/id_ed25519 signing/signing_key
# Add the public key to GitHub: Settings → SSH and GPG keys → "Signing Key"
```

**How it works:** The `./signing/` directory is unconditionally mounted into the agent container at `/home/agent/.ssh/signing/` (read-only). At startup the agent scans for key files (`signing_key`, `id_ed25519`, `id_ecdsa`, `id_rsa`). If found, it copies the key to a writable location, fixes permissions to 0600, and configures `gpg.format=ssh`, `commit.gpgsign=true`, `tag.gpgsign=true`. If the directory is empty, signing is silently skipped.

**Optional `allowed_signers`:** Drop an `allowed_signers` file in the same directory for signature verification.

**Override:** For non-standard key paths, use `signing:` in config.yaml or `GIT_SIGNING_KEY` env var.

The `signing/` directory is gitignored.

## CLI Backends

Per-task backend + model selection. Users pick CLI + model when creating tasks in the UI. Config defines available backends:

```yaml
agent:
  backends:
    claude:
      models: [opus, sonnet, haiku]
      default_model: sonnet
    codex:
      models: [gpt-5.4, gpt-5.4-mini, gpt-5.3-codex]
      default_model: gpt-5.4
```

| Backend | Binary | Auth | Models | MCP Support | Project Instructions |
|---------|--------|------|--------|-------------|---------------------|
| `claude` | `claude` | `~/.claude` (volume mount) | opus, sonnet, haiku | Yes (`--mcp-config`) | CLAUDE.md |
| `codex` | `codex` | `~/.codex` (volume mount) | gpt-5.4, gpt-5.4-mini, gpt-5.3-codex | No | AGENTS.md |

**Per-task columns:** `agent_tasks.cli` + `agent_tasks.model` (both nullable, default from config). Subtasks inherit parent's cli/model.

**Dispatch flow:** `cli::run(task.cli, opts)` → `claude::run()` or `codex::run()`

**AGENTS.md symlink:** Automatically created as `AGENTS.md → CLAUDE.md` in every repo/worktree so Codex can read the same project instructions. Created by `workspace::ensure_agents_md_symlink()` after clone and worktree creation.

**Key files:**
- `agent/src/cli.rs` — Common `CliOptions`/`CliResult` types + dispatch
- `agent/src/claude/mod.rs` — Claude Code CLI (`-p --dangerously-skip-permissions --output-format json`)
- `agent/src/codex/mod.rs` — Codex CLI (`--full-auto --quiet`)
- `agent/src/config.rs` — `Backends` (IndexMap), `default_cli()`, `default_model(cli)`

## Agent Instructions

Optional markdown file that gives the agent persistent context across all tasks. Configured via `agent.instructions` in config.yaml (path to a `.md` file, relative to config directory). Loaded once at startup, injected as `--system-prompt` for Claude Code or prepended to prompt for Codex on every invocation (planning, implementation, feedback).

Use this for:
- **Agent identity** — who it is, what project it's working on
- **Repo map** — what each configured repo does and how they relate
- **Cross-repo conventions** — shared patterns, naming, testing requirements
- **Architectural context** — things not derivable from a single repo's CLAUDE.md

```yaml
agent:
  instructions: instructions.md   # relative to config.yaml
```

See `instructions.example.md` for the recommended format.

**Flow:** `config.rs` loads file content → stored as `Config.instructions: Option<String>` → passed as `CliOptions.system_prompt` in planner/implementer → Claude backend merges with its hardcoded `AGENT_SYSTEM_PROMPT`, Codex backend prepends to full prompt.

**Key distinction from CLAUDE.md:** Per-repo CLAUDE.md files contain repo-specific instructions (code patterns, test commands). The instructions file contains cross-repo context that no single CLAUDE.md can provide — the agent's identity, how repos relate, and system-wide conventions.

## Repo Onboarding (Auto-Discovery)

When repos are added (via config.yaml or the UI), the agent automatically profiles them:

1. **Startup sync**: `onboarding::sync_repo_profiles()` checks config repos against DB profiles, creates `pending` rows for new ones
2. **Onboarding**: For each pending profile, invokes the CLI (haiku model, read-only tools) with `templates/onboard_prompt.txt`
3. **Profile storage**: Parses structured JSON (`<!-- profile ... profile -->` block) + summary from CLI output, stores in `repo_profiles` table
4. **Staleness**: `check_stale_profiles()` compares stored commit SHA vs HEAD; marks profiles as `stale` when repos update

### Smart Multi-Repo Planning

When all repos have ready profiles, the planner switches to "smart mode":
- Injects repo summaries into the planning prompt (via `onboarding::format_profiles_for_prompt()`)
- Only clones the first repo for working_dir context (instead of all repos)
- Tells Claude which repos do what, so it can decide which need changes
- Falls back to full exploration if any profile is missing

### Profile JSON Structure
```json
{
  "language": "rust",
  "framework": "axum",
  "package_manager": "cargo",
  "test_command": "cargo test",
  "lint_command": "cargo clippy",
  "build_command": "cargo build",
  "entry_points": ["src/main.rs"],
  "key_directories": {"src/agent/": "Core agent logic"},
  "ci_workflows": ["ci.yml"],
  "has_claude_md": true,
  "has_mcp_config": false,
  "dependencies_summary": "tokio, axum, sqlx, redis"
}
```

### UI Repo Management

Frontend "Repos" tab (`home.tsx` → `ReposPage`):
- Lists all repo profiles with status badges (pending/onboarding/ready/failed/stale)
- Add repos via dialog (owner/repo format + branch)
- View profile details (language, framework, commands, directories, summary)
- Trigger re-onboarding, delete repos
- 5s polling for status updates

### Key files
- `agent/src/onboarding.rs` — Core onboarding logic (sync, onboard, stale check, prompt formatting)
- `agent/templates/onboard_prompt.txt` — CLI prompt for repo exploration
- `agent/src/api/mod.rs` — `/repos` endpoints (list, add, delete, trigger onboard)
- `migrations/010_repo_profiles.sql` — DB schema
- `skills/add-repo.md` — Skill file for manual triggering
- `frontend/components/agent/repos-page.tsx` — Repos UI page
- `frontend/lib/repos.ts` — Types + API client

## Skills

9 built-in skills in `skills/` (auto-loaded at startup):

| Skill | Phase | Purpose |
|-------|-------|---------|
| `test` | implement | Auto-detect test framework, run tests |
| `lint` | implement | Auto-detect linter, run with auto-fix |
| `typecheck` | implement | Run static type checking |
| `commit` | implement | Conventional commit format guide |
| `review` | implement | Self-review checklist before PR |
| `build` | implement | Auto-detect build system, verify build |
| `migrate` | both | Database migration guidance (Supabase, Prisma, Drizzle, Alembic, etc.) |
| `security` | implement | Security scan and dependency audit |
| `add-repo` | both | Onboard a repository — explore structure, generate profile summary |

Skills are injected into the CLI prompt as context (works with both Claude and Codex). Each skill has:
- `description` — what it does
- `command` — optional shell command to run
- `prompt` — additional instructions for Claude
- `phase` — when to use: `plan`, `implement`, or `both`

**Three sources (merged at runtime):**
1. `config.yaml` inline skills → global, highest precedence
2. `skills/` directory next to config → global
3. `repo/skills/*.md` → auto-discovered per repo

Skill file format: markdown with YAML frontmatter (---, description, command, phase, ---).

## MCP Servers

Default MCP servers enabled (no API keys needed):
- **fetch** — fetch any URL as clean markdown
- **context7** — up-to-date library documentation
- **memory** — persistent knowledge graph across tasks
- **sequential-thinking** — structured multi-step reasoning
- **github** — full GitHub API (uses existing GITHUB_TOKEN)

Optional servers (need API keys in .env):
- **sentry** — error context for bug fixes
- **linear** — task details and acceptance criteria
- **slack** — post updates, ask questions
- **postgres** — schema inspection, read-only queries
- **supabase** — project management
- **notion** — project docs and wikis
- **brave-search** — web search
- **playwright** — browser automation for testing

Repos can also provide `.mcp.json` at their root — these are auto-discovered and merged with the global config at runtime. Global servers take precedence on name conflicts.

## Rules

- Tailwind classes only, no vanilla CSS
- No shadcn component modifications — use className overrides
- API routes validate auth via `auth()` before any DB query
- Agent backend uses service-level DB access (no row-level security)
- One task at a time per worker (configurable via max_concurrent_tasks)
