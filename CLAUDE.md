# Karna — Self-Hosted Autonomous Coding Agent

Create tasks on a kanban board, an AI agent plans and implements them, opens PRs on GitHub, and notifies you via email. You review plans and PRs, provide feedback, and the agent iterates.

## Architecture

```
Cargo workspace: agent/, api/, shared/

docker-compose.yml
├── postgres:16     — Auth.js sessions + agent_tasks + agent_logs
├── redis:7         — Task queue + distributed locks
├── api (Rust/Axum) — REST API, all CRUD + JWT auth, serves frontend data
├── agent (Rust)    — Polls DB, invokes Claude Code or Codex CLI, git/gh operations
├── frontend (Next.js) — Kanban board UI, Auth.js login only, proxies /api to Rust API
├── code-server     — Browser IDE with dev tooling (git, gh, claude, codex) + config-driven extensions
├── tunnel          — Optional Cloudflare Tunnel for public access
└── autoheal        — Auto-restarts unhealthy containers
```

### Crate structure
- **shared/** (`karna-shared`) — Database client (sqlx) + domain models (AgentTask, Schedule, RepoProfile, etc.)
- **api/** (`karna-api`) — Axum REST server, NextAuth JWE decryption, CORS, all data endpoints
- **agent/** (`karna-agent`) — Task orchestration, CLI backends, git operations, scheduler, webhooks

## Tech Stack

### Frontend (`frontend/`)
- **Framework**: Next.js 15 (App Router), React 19, TypeScript
- **Auth**: Auth.js v5 + email/password credentials + @auth/pg-adapter (login/signup only)
- **Styling**: Tailwind CSS 3 + shadcn/ui
- **DnD**: @dnd-kit/core + @dnd-kit/sortable
- **Animations**: Framer Motion
- **State**: TanStack Query + 5s polling with AbortSignal
- **Data**: All `/api/*` requests (except auth) proxied to Rust API via Next.js rewrites — no direct DB access

### API (`api/`)
- **Language**: Rust (2021 edition)
- **Runtime**: Tokio
- **HTTP**: Axum + tower-http (CORS)
- **DB**: karna-shared (sqlx Postgres)
- **Auth**: NextAuth v5 JWE decryption (HKDF-SHA256 + A256CBC-HS512)
- **Queue**: Redis (schedule triggers)

### Agent Backend (`agent/`)
- **Language**: Rust (2021 edition)
- **Runtime**: Tokio
- **HTTP**: Axum (health + webhooks)
- **DB**: karna-shared (sqlx Postgres)
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
│   ├── 011_cancelled_status.sql # Cancelled task status
│   ├── 012_task_attachments.sql # Task image attachments
│   ├── 013_repo_sync_issues.sql # Per-repo GitHub issue sync toggle
│   ├── 014_assignee_and_external.sql # Human assignment + Linear/ClickUp source fields
│   ├── 015_webhook_status.sql   # Per-repo webhook registration outcome (status/url/error)
│   ├── 016_agent_profiles.sql   # Named agent identities + agent_tasks.assigned_agent_id
│   ├── 017_pr_reviews.sql       # pr_reviews + repo_profiles.review_prs / .review_agent_id
│   ├── 018_policies.sql         # Policies (advisory plan-review guardrails) + agent_tasks.policy_matches
│   ├── 019_pr_review_logs.sql   # Per-review activity log for live progress streaming
│   ├── 020_pr_review_findings.sql # Per-(path, line) findings for inline review comments
│   └── 021_pr_review_finding_severity.sql # Severity tier (high/medium/low) per finding
├── Cargo.toml                   # Workspace root (members: agent, api, shared)
├── shared/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs               # Re-exports db + models
│       ├── db.rs                # Database client (sqlx) — all CRUD queries
│       └── models.rs            # Domain models: AgentTask, Schedule, RepoProfile, etc.
├── api/
│   ├── Cargo.toml
│   ├── Dockerfile
│   └── src/
│       ├── main.rs              # Axum server, router, CORS, state
│       ├── auth.rs              # NextAuth v5 JWE decryption (HKDF + A256CBC-HS512)
│       ├── config.rs            # Reads config.yaml for repos/backends/skills
│       └── routes/
│           ├── tasks.rs         # CRUD, logs, comments, subtasks
│           ├── schedules.rs     # CRUD, trigger, runs, run logs
│           ├── repos.rs         # List, add, delete, onboard
│           ├── users.rs         # List users (for assignee dropdown)
│           ├── webhooks.rs      # GitHub + Linear + ClickUp webhook handlers
│           └── config.rs        # Config endpoint
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
│       ├── opencode/mod.rs      # opencode CLI runner (any model via OpenRouter)
│       └── updater.rs           # Self-repo change detection + classification
├── frontend/
│   ├── Dockerfile
│   ├── package.json
│   ├── next.config.mjs          # Rewrites /api/* (except auth) to Rust API
│   ├── auth.ts                  # Auth.js config (credentials provider + pg adapter)
│   ├── middleware.ts             # Route protection
│   ├── app/
│   │   ├── layout.tsx           # Root layout with SessionProvider
│   │   ├── globals.css          # Tailwind + CSS variables (dark theme)
│   │   ├── page.tsx             # Kanban board (6 columns, DnD, polling)
│   │   ├── login/page.tsx       # Email/password login + signup
│   │   └── api/auth/            # Auth.js routes only — all other /api proxied to Rust API
│   ├── lib/
│   │   ├── db.ts                # pg Pool (auth only)
│   │   ├── agent-tasks.ts       # Types + API client helpers (with AbortSignal)
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
  - `assignee_user_id` (UUID, nullable) — NULL = agent picks up; set = assigned to a human (agent skips)
  - `assigned_agent_id` (UUID, nullable) — FK to `agent_profiles.id`. NULL = any agent profile picks it up; set = only that named agent profile picks it up
  - `policy_matches` (JSONB, nullable) — policies that fired against this task's plan: `[{policy_id, name, severity, message, paths: [...]}]`. Populated by the planner after `set_plan`; rendered as a banner on the plan_review tab
  - `external_source` (TEXT, nullable) — origin if ingested ("linear" or "clickup")
  - `external_id` (TEXT, nullable) — ID in the external system (unique with source)
  - `external_url` (TEXT, nullable) — direct link to the external task
- `agent_logs` — Append-only agent activity log per task (includes user comments with `log_type = 'comment'`)
- `agent_profiles` — Named agent identities (one per `(cli, model)` from config, auto-seeded on startup); see "Agent Profiles" section below
- `policies` — Advisory plan-review guardrails: `(name, repo_pattern, path_glob, message, severity, enabled)`; see "Policies" section below

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
  - `sync_issues` (BOOLEAN) — when TRUE, GitHub issues on this repo become tasks
  - `review_prs` (BOOLEAN) — when TRUE, the agent auto-reviews human-opened PRs
  - `review_agent_id` (UUID, nullable) — FK to `agent_profiles.id`; NULL = use config defaults
- `pr_reviews` — One row per (repo, head_sha) PR review attempt; UNIQUE constraint dedupes concurrent webhook firings; tracks status, reviewer agent, comments_posted, cost_usd
- `pr_review_logs` — Append-only per-review activity log streamed live from the CLI (tool calls, assistant text, errors). Powers the live progress modal
- `pr_review_findings` — Per-(path, line) findings emitted by the reviewer CLI. `posted=true` rows became inline review comments via GitHub's Reviews API; `posted=false` rows didn't survive anchor validation (with `skip_reason`). Each finding carries `severity` (high/medium/low) which drives both the inline comment-body marker on GitHub and the badge color in the UI. Surfaced in the review-log-modal so reviewers can see both what landed inline and what got dropped

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
- Both parent and subtask cards render on the kanban — `nestSubtasks()` annotates parents with `.subtasks`/`subtask_count` but returns the full flat list, and `getTasksForColumn()` defaults `includeSubtasks=true`
- The TaskCard shows a progress bar for parent tasks (X/N subtasks complete)
- The TaskDetailModal shows a "Subtasks" tab with per-subtask status, repo, and PR links
- Deleting a parent cascades to all subtasks (ON DELETE CASCADE)

## API Routes

All data routes are served by the Rust API (`api/`). The frontend proxies `/api/*` (except auth) via Next.js rewrites.

**Rust API (karna-api, :8081):**

| Method | Path | Purpose |
|--------|------|---------|
| GET | /health | Health check |
| GET | /api/tasks | List all tasks for current user |
| POST | /api/tasks | Create new task (repo optional for multi-repo) |
| PATCH | /api/tasks/{id} | Update task fields |
| DELETE | /api/tasks/{id} | Delete task |
| GET | /api/tasks/{id}/logs | Get agent logs for task (capped at 200) |
| POST | /api/tasks/{id}/comments | Post a comment (creates log entry + sets feedback for agent) |
| GET | /api/tasks/{id}/subtasks | List subtasks for a parent task |
| POST | /api/tasks/{id}/subtasks | Parse plan & create subtasks (plan approval) |
| GET | /api/schedules | List all schedules for current user (with last run) |
| POST | /api/schedules | Create new schedule (cron or one-shot) |
| GET | /api/schedules/{id} | Get single schedule |
| PATCH | /api/schedules/{id} | Update schedule fields (name, prompt, enabled, etc.) |
| DELETE | /api/schedules/{id} | Delete schedule (cascades to runs + logs) |
| POST | /api/schedules/{id}/trigger | Manual trigger (sets Redis key for immediate pickup) |
| GET | /api/schedules/{id}/runs | List runs for a schedule |
| GET | /api/schedules/{id}/runs/{runId}/logs | Get logs for a specific run (capped at 200) |
| GET | /api/repos | List all repo profiles |
| POST | /api/repos | Add new repo (triggers onboarding) |
| DELETE | /api/repos/{id} | Delete repo profile |
| POST | /api/repos/{id}/onboard | Trigger re-onboarding for a repo |
| POST | /api/repos/{id}/webhook | Force webhook re-registration on next agent poll |
| GET | /api/repos/{id}/reviews | List recent PR reviews for this repo (50, Redis-cached) |
| GET | /api/repos/{id}/reviews/{review_id}/logs | Live activity log for one review (200, short TTL) |
| GET | /api/repos/{id}/reviews/{review_id}/findings | Per-(path, line) findings for one review (posted + skipped) |
| GET | /api/users | List users (id, name, email) for assignee dropdown |
| GET | /api/agents | List agent profiles (Redis-cached) |
| POST | /api/agents | Create custom agent profile |
| PATCH | /api/agents/{id} | Update profile (rename, pause/unpause, set default) |
| DELETE | /api/agents/{id} | Delete profile (tasks fall back to "any agent") |
| GET | /api/agents/{id} | Single agent profile |
| GET | /api/agents/{id}/stats | Aggregate counts: total/open tasks, PRs opened, reviews, rolling cost |
| GET | /api/agents/{id}/tasks | Recent tasks assigned to this agent (50, by updated_at) |
| GET | /api/agents/{id}/reviews | Recent PR reviews this agent ran (50, by created_at) |
| GET | /api/assignables | Unified list of agents + humans for the assignee picker |
| GET | /api/policies | List all policies (Redis-cached) |
| POST | /api/policies | Create a policy |
| PATCH | /api/policies/{id} | Update name / glob / message / severity / enabled |
| DELETE | /api/policies/{id} | Delete a policy |
| GET | /api/config | Config (repos, backends, skills, MCP servers) |

**Frontend (Next.js, :3000) — auth only:**

| Method | Path | Purpose |
|--------|------|---------|
| GET/POST | /api/auth/* | Auth.js handlers (login, session, CSRF) |
| POST | /api/auth/signup | User registration |

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

## Task Assignment (Agent vs Human)

Every task has an `assignee_user_id`. NULL = the agent picks it up (default, original behavior). Set to a user UUID = the task belongs to that human, and the agent must skip it.

**Where the filter lives:** `next_actionable_task()`, `active_task_ids()`, and `tasks_with_pending_feedback()` in [shared/src/db.rs](shared/src/db.rs) all add `AND assignee_user_id IS NULL`. No code in `agent/` needs to know about assignees — the DB filter is the single gate.

**UI:**
- Create dialog: "Assigned to" picker — toggle between "Agent" and a human dropdown ([tasks/new/page.tsx](frontend/app/(dashboard)/tasks/new/page.tsx))
- Task card: blue "Human" badge when assigned ([components/agent/task-card.tsx](frontend/components/agent/task-card.tsx))
- Task detail modal: editable assignee selector in the Details tab; reassign back to Agent at any time to resume agent work

**User list source:** `GET /api/users` (filters out the default system user — that ID represents "no human", not a person).

## Agent Profiles (Named Pseudo-Users)

Agents have identities. Instead of an anonymous "the agent," each (cli, model) pair from `config.yaml` becomes a named pseudo-user (e.g. "Claude Sonnet", "Codex GPT-5.4") that shows up next to humans in the assignee dropdown. A task can be assigned to a specific agent profile and only that profile picks it up.

**Table:** [agent_profiles](migrations/016_agent_profiles.sql) — `(id, slug UNIQUE, name, avatar_emoji, cli, model, system_prompt_addendum, paused_reason, is_default)`. Slug is the natural key (e.g. `claude-sonnet`) so renames don't break startup seeding.

**Assignment semantics on `agent_tasks`:**

| `assignee_user_id` | `assigned_agent_id` | Behavior |
|---|---|---|
| SET | any | Human owns it; agent skips (existing behavior) |
| NULL | NULL | Any active agent profile picks it up (existing default) |
| NULL | SET | Only that specific agent profile picks it up; if it's paused, the task waits |

**Pickup filter** (`next_actionable_task` / `active_task_ids` / `tasks_with_pending_feedback` in [shared/src/db.rs](shared/src/db.rs)): `AND (assigned_agent_id IS NULL OR EXISTS (SELECT 1 FROM agent_profiles p WHERE p.id = t.assigned_agent_id AND p.paused_reason IS NULL))`. The worker is generic — any agent process can run any profile's `(cli, model)`. Pausing a profile (setting `paused_reason`) blocks pickup of its tasks without affecting tasks assigned to other profiles.

**Runtime resolution** ([agent/src/agent/mod.rs](agent/src/agent/mod.rs) — `resolve_runtime`): precedence is `task.cli/.model` → `assigned_profile.cli/.model` → `config.default_cli/.default_model`. The profile's `system_prompt_addendum` is appended to the global instructions file at CLI invocation time via `merge_system_prompt`, so per-agent personas (style, focus areas) stack on top of the cross-repo instructions file.

**Auto-seeding** ([agent/src/profiles.rs](agent/src/profiles.rs)): on agent startup, `seed_from_config` inserts one row per `(cli, model)` from `config.agent.backends`, slugged and titled (e.g. "Claude Sonnet"). Idempotent on slug — existing rows are left alone, so user renames stick. The default profile is the `(default_cli, default_model)` pair.

**API:**

| Method | Path | Purpose |
|---|---|---|
| GET | /api/agents | List all profiles (Redis-cached, key `cache:agents:list`) |
| POST | /api/agents | Create a custom profile (e.g. a Sonnet persona with a strict style addendum) |
| PATCH | /api/agents/{id} | Rename, change emoji/cli/model, pause/unpause, set as default |
| DELETE | /api/agents/{id} | Delete (FK is `ON DELETE SET NULL` so tasks drop back to "any agent") |
| GET | /api/assignables | Unified `{type: "agent"\|"user", ...}` list for the frontend assignee picker |

**Frontend:** the assignee picker in [tasks/new/page.tsx](frontend/app/(dashboard)/tasks/new/page.tsx) and [task-detail-modal.tsx](frontend/components/agent/task-detail-modal.tsx) is a single `<select>` with optgroups (`Any agent` default, then `Agents`, then `Humans`). Paused profiles render as disabled options. TaskCard shows a purple badge with the assigned agent's emoji + name when set; amber when the assigned agent is paused. Encoded picker value is `""` / `agent:<id>` / `user:<id>` for serialization through to the API.

## Policies (Plan-Review Guardrails)

Advisory rules that surface a banner on the plan_review tab when a task's plan touches sensitive paths. Today the agent does not gate the state transition — the human reviewer still controls approve/reject. Severity `block` is reserved for future enforcement and currently renders the same as `warn`.

**Schema** ([migrations/018_policies.sql](migrations/018_policies.sql)):

| Field | Notes |
|---|---|
| `repo_pattern` | `*` matches all, `owner/*` is an owner prefix, `owner/repo` is exact |
| `path_glob` | Standard glob with `**` (any segments) + `*` (chars except `/`) |
| `severity` | `warn` (amber banner) or `block` (red banner; future: dims approve button) |
| `enabled` | Disabled policies are skipped by the scan |

**Scan flow** ([agent/src/policies.rs](agent/src/policies.rs)):

1. `planner` calls `policies::scan_and_persist` after `set_plan`.
2. `extract_paths` parses the plan markdown for tokens that look like file paths (anything containing `/`, with URL/version noise filtered out, trailing `:line:col` stripped).
3. For each active policy: `repo_match` against the task's repos (multi-repo parent only matches `*`), then `glob_match` against each extracted path.
4. Hits are persisted on `agent_tasks.policy_matches` as JSON: `[{policy_id, name, severity, message, paths: [...]}]`.

The scan is *advisory and failure-tolerant* — errors are logged and the planner continues. The path extractor is intentionally over-inclusive: better an extra banner than a missed migration.

**UI:**
- `/policies` page ([app/(dashboard)/policies/page.tsx](frontend/app/(dashboard)/policies/page.tsx)) — list, create, toggle, delete. Inline create form (no separate dialog).
- Banner on the Plan tab of [task-detail-modal.tsx](frontend/components/agent/task-detail-modal.tsx) when `task.policy_matches` is non-empty. Amber for `warn`, red for `block`. Shows name, severity, message, and the first 3 matched paths.

## Agent Profile Pages

Each named agent gets a real profile page at `/agents/{id}` so they feel like teammates instead of an anonymous process.

- `/agents` ([app/(dashboard)/agents/page.tsx](frontend/app/(dashboard)/agents/page.tsx)) — index of all profiles. Shows avatar emoji, name, cli/model, default flag, paused state. Link target for the agent badge on task cards.
- `/agents/{id}` ([app/(dashboard)/agents/[id]/page.tsx](frontend/app/(dashboard)/agents/[id]/page.tsx)) — detail page with:
  - Header: emoji, name, cli/model, default/paused badges, Pause/Resume + Set default + Edit actions.
  - Stats row: total tasks, open tasks, PRs opened, reviews done, rolling cost_usd (theoretical quota burn; not a real bill — see Agent Profiles cost note).
  - Edit form: rename, change emoji/cli/model, set `system_prompt_addendum` (extra prompt that's stacked on the global instructions file for this agent's runs).
  - Recent tasks (50): linked back to `/tasks/{id}`.
  - Recent PR reviews (50): linked to GitHub PR URLs.

**Backing endpoints:** `GET /api/agents/{id}`, `/api/agents/{id}/stats`, `/api/agents/{id}/tasks`, `/api/agents/{id}/reviews`. The stats endpoint joins on `agent_tasks` + `pr_reviews` so the rolling cost is the sum across both the implementer and reviewer paths.

## Shared Workspace Mode (Small Teams)

Set `KARNA_SHARED_WORKSPACE=true` (env on `api` + `agent`) to make Karna a single shared team workspace. Every signed-in user can see and edit every task, schedule, and repo — regardless of `user_id`. Auth is still required at login; pair with `SIGNUP_DISABLED=true` so only invited users can join.

**Where the flag lives:** [shared/src/db.rs](shared/src/db.rs) — `Database::with_shared_workspace(bool)` on the builder, checked by every user-scoped query (`list_tasks_for_user`, `update_task`, `delete_task`, `task_belongs_to_user`, `list_schedules_for_user`, `update_schedule_fields`, `delete_schedule`, `schedule_belongs_to_user`, `count_open_tasks_with_prefix`, `max_prefix_number`). When set, the `WHERE user_id = ...` clause is dropped entirely.

**Wiring:** `api/src/main.rs` and `agent/src/main.rs` read `KARNA_SHARED_WORKSPACE` and call `.with_shared_workspace(...)` on the `Database` builder. The `task_belongs_to_user` filter in `api/src/routes/tasks.rs` (post_comment / create_subtasks) checks `state.db.is_shared_workspace()` before falling through. `/api/config` exposes `sharedWorkspace: bool` so the frontend can adjust UX.

**Frontend signal:** When `sharedWorkspace=true`, task cards display a "creator" badge (`User` icon + name) whenever the task wasn't created by the current viewer — so people know whose task they're touching. Implemented in [components/agent/task-card.tsx](frontend/components/agent/task-card.tsx) via the `creatorLabel` prop, resolved in [(dashboard)/page.tsx](frontend/app/(dashboard)/page.tsx) using `useUsers()` and the current session user id.

**Cache:** Cache keys remain per-user (`cache:tasks:list:{user_id}`) but content is identical across users in shared mode. Pattern invalidation already busts all keys on writes, so correctness is preserved. The redundancy is bounded by user count.

**Helm:** `auth.sharedWorkspace: false` (default). Set to `true` to inject `KARNA_SHARED_WORKSPACE=true` into both `api` and `agent` deployments.

## External Task Sources (Linear / ClickUp Ingest)

Tasks can be ingested from Linear or ClickUp via webhooks. Each task records its origin in `external_source` / `external_id` / `external_url`. When the agent opens a PR, it posts a backlink onto the external task so the source system stays in sync.

### Webhooks

| Source | Endpoint | Header | Secret env | Event handled |
|--------|----------|--------|-----------|---------------|
| Linear | `POST /webhooks/linear` | `linear-signature` (hex HMAC-SHA256) | `LINEAR_WEBHOOK_SECRET` | `action: create, type: Issue` |
| ClickUp | `POST /webhooks/clickup` | `x-signature` (hex HMAC-SHA256) | `CLICKUP_WEBHOOK_SECRET` | `event: taskCreated` |

Both use HMAC-SHA256 against the raw body (no `sha256=` prefix, unlike GitHub). If the corresponding secret env is unset, signatures are accepted without verification (useful for local testing).

**Dedupe:** `find_task_by_external(source, external_id)` is called before creating a task. The `(external_source, external_id)` unique index in migration 014 backs this.

**Linear payload** is rich enough to populate title, description, priority, and URL directly.

**ClickUp payload** is sparse (just `task_id` + `history_items`). When `CLICKUP_API_TOKEN` is set, the handler calls `GET /api/v2/task/{id}?include_markdown_description=true` to enrich title, markdown description, URL, and priority. Without the token, it falls back to whatever the payload contains (`history_items` may include a "name" entry), and finally to a `ClickUp task <id>` placeholder.

### PR Backlinks (Agent → Linear/ClickUp)

When `set_pr()` fires, the agent calls `external::notify_pr_opened()` ([agent/src/external.rs](agent/src/external.rs)). If the task has an `external_source`, the agent posts a comment with the PR URL on the originating Linear issue / ClickUp task.

**API tokens required:**
- `LINEAR_API_KEY` — used against `https://api.linear.app/graphql` (`commentCreate` mutation)
- `CLICKUP_API_TOKEN` — used against `https://api.clickup.com/api/v2/task/{id}/comment`

If the token is missing, the agent logs a debug-level message and continues. Backlink failures never fail the PR.

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

**Auto-registration:** Webhooks are automatically registered on repos during onboarding when a public URL is available. The agent derives `webhook_url` from: `AGENT_WEBHOOK_URL` env → `TUNNEL_AGENT_HOSTNAME` env (prefixed with `https://`) → None. If available, `onboard_repo()` calls `github::ensure_repo_webhook()` after profiling completes. Idempotent — checks existing hooks first. Requires `admin:repo_hook` scope on `GITHUB_TOKEN`.

The outcome is persisted on `repo_profiles.webhook_status` (`registered` | `failed` | `unsupported` | `not_registered`) plus `webhook_error` and `webhook_url`. The Repos UI surfaces this so users see the difference between "issue sync flag is on" and "webhook is actually live" — see migration `015_webhook_status.sql`.

**Reconciler:** `onboarding::reconcile_webhooks()` runs every poll cycle (see `agent/src/main.rs`). For every `ready` repo with `sync_issues=true` whose webhook isn't `registered` (or points to a stale URL), it retries registration. This means:
- Toggling `sync_issues` on later → webhook gets installed automatically on next poll.
- Setting `AGENT_WEBHOOK_URL` after startup → existing repos pick it up without re-onboarding.
- Webhook failures (e.g. missing `admin:repo_hook` scope) keep getting retried after you rotate the token.

**UI feedback** ([components/agent/repo-card.tsx](frontend/components/agent/repo-card.tsx), [components/agent/repo-detail-modal.tsx](frontend/components/agent/repo-detail-modal.tsx)): the "issues" badge turns amber when sync is on but no webhook is live; the detail modal shows a dedicated webhook status row with the underlying reason ("No public URL configured", "Webhook registration failed: …"). `/api/config` exposes `webhookUrlConfigured: bool` so the frontend can globally surface when no `AGENT_WEBHOOK_URL` / `TUNNEL_AGENT_HOSTNAME` is set.

**Signature verification:** When `GITHUB_WEBHOOK_SECRET` is set, the handler verifies `X-Hub-Signature-256` using HMAC-SHA256. If no secret is configured, all payloads are accepted (verification disabled). The same secret is passed to GitHub when auto-registering webhooks during onboarding.

**Key files:**
- `agent/src/api/mod.rs` — Webhook handler + HMAC-SHA256 verification
- `agent/src/agent/mod.rs` — Poll loop feedback detection
- `agent/src/agent/implementer.rs` — Feedback application
- `agent/src/git/github.rs` — PR comment gathering + `ensure_repo_webhook()` (auto-registration)
- `agent/src/onboarding.rs` — Calls webhook registration after repo profile is stored
- `agent/src/config.rs` — `webhook_url` derivation (AGENT_WEBHOOK_URL → TUNNEL_AGENT_HOSTNAME → None)
- `frontend/app/api/tasks/[id]/comments/route.ts` — Frontend comment → feedback path

## Auto-Review of Human-Opened PRs

When a teammate opens a PR (or force-pushes new commits) on a repo with `review_prs = TRUE`, the agent runs a read-only review and posts a single review comment via `gh pr review`. Uses your existing Claude/Codex subscription — no extra cost.

**Trigger + execution are decoupled.** Whichever process receives the webhook only *enqueues* a `pending` row on `pr_reviews`; the agent's main poll loop claims pending rows and runs the actual CLI review. This makes cloud and local-dev deployments behave identically:

| Deployment | Webhook lands on | Enqueues via | Picks up via |
|---|---|---|---|
| Helm (production) | `karna-api:8081` ([api/src/routes/webhooks.rs](api/src/routes/webhooks.rs)) | `db.enqueue_pr_review` | Agent poll loop `reviewer::run_pending_reviews` |
| docker-compose (local) | Either karna-api **or** agent's own port ([agent/src/api/mod.rs](agent/src/api/mod.rs)) | `reviewer::enqueue_review` | Same agent poll loop |

The `pull_request` event was already subscribed (no webhook re-registration needed). Branch filter routes PR open / synchronize / reopened events:

- Branch starts with `{prefix}-{N}/` (agent's own PR) → existing feedback path
- Otherwise → human-opened PR → `handle_pr_review_trigger` enqueues a `pending` row, returns immediately

**Per-repo opt-in.** Reviews are off by default. Two toggles on `repo_profiles`:

- `review_prs BOOLEAN` — opt-in. Surfaced as a switch in the repo detail modal under "Settings."
- `review_agent_id UUID NULL` — which agent profile reviews PRs for this repo. NULL means use config defaults. Recommended for cost control: pick a cheap model (e.g. haiku / gpt-5.4-mini).

**Skipped automatically:**
- Drafts (`pull_request.draft = true`)
- Branches starting with `{prefix}-{N}/` (agent's own PRs go through the implementer feedback path, not the reviewer)
- Repos without `review_prs = TRUE`
- Re-runs on the same `head_sha` — UNIQUE `(repo, head_sha)` on `pr_reviews` dedupes concurrent webhook firings race-safely. Force-pushes (new SHA) get a fresh review.

**Reviewer module** ([agent/src/reviewer.rs](agent/src/reviewer.rs)):

- Runs the CLI with `allowed_tools = "Read,Glob,Grep,Bash"` (no Edit/Write).
- System prompt locks the agent to substantive issues only: bugs, security, correctness, missing edge cases. Style/naming/lint-equivalents are explicitly forbidden. If the diff is clean, the agent posts a brief "looks good" summary with zero inline comments rather than manufacturing findings.
- Agent uses `gh pr diff <pr>` + `gh pr view <pr> --json files,title,body` to see the change and reads surrounding code with Read/Glob/Grep. It does NOT post the review itself — instead, its final assistant message ends with a `<!-- findings ... findings -->` JSON block containing a `summary` string + a `comments[]` array of `{path, line, side, start_line?, body}` objects.
- `--approve` / `--request-changes` are forbidden — review is comment-only so humans always own the merge decision.
- Working dir is the pre-cloned `repos_dir/<repo>` (no per-review checkout; review is read-only).

**Structured findings → inline comments** (replaces the old `gh pr review --body` flow):

1. `parse_findings` (in [agent/src/reviewer.rs](agent/src/reviewer.rs)) extracts the `<!-- findings ... findings -->` block from the CLI's `result.output`. If parsing fails, the raw output is posted as a body-only review (fallback).
2. `fetch_diff` runs `gh pr diff <pr> --repo <repo>` to get the unified diff, and `DiffIndex::parse` walks each hunk to build a `HashMap<(path, side), HashSet<line>>` of GitHub-acceptable anchors. Side `R` covers added + context lines (post-change line numbers); side `L` covers removed + context lines (pre-change line numbers).
3. `validate_findings` filters every finding: line numbers not present in the diff, multi-line ranges with `start_line > line`, and empty bodies/paths get dropped to a `skipped` list with a `skip_reason` string.
4. Every finding (posted-eligible + skipped) is persisted to `pr_review_findings` via `db.insert_pr_review_finding` so the UI can render both.
5. `post_structured_review` POSTs once to `repos/{owner}/{repo}/pulls/{n}/reviews` via `gh api --input -` with `{event: "COMMENT", body, comments: [...]}`. The `comments[]` payload uses GitHub's review-comment shape (`path`, `line`, `side`, optional `start_line`/`start_side` for multi-line). If the POST fails entirely, a body-only review is posted as a last-resort fallback.
6. Any skipped findings get appended to the body as a "couldn't anchor to the diff" footer so the PR author still sees the underlying concern.

The validator is intentionally strict — GitHub rejects the *entire* review submission if a single inline comment anchors to a line outside the diff, so it's cheaper to drop a finding than to lose the whole review.

**`pr_review_findings` table** ([migrations/020_pr_review_findings.sql](migrations/020_pr_review_findings.sql)):

| Column | Purpose |
|---|---|
| `(path, line, start_line, side)` | Anchor on the PR diff. `side='RIGHT'` is the common case (additions/context on the new file); `side='LEFT'` only for comments on removed lines. `start_line` is set for multi-line ranges; NULL for single-line. |
| `body` | Markdown body of the inline comment. |
| `severity` | `high` / `medium` / `low`. `normalize_severity` coerces variants like `"HIGH"`, `"Sev: High"`, `"critical"`, `"nit"` into the canonical set so a forgetful model never trips the CHECK constraint. `severity_marker` prepends a colored emoji + bold label (`🔴 Sev: High`, `🟡 Sev: Medium`, `🔵 Sev: Low`) to the body when posting to GitHub so the tier shows on github.com without depending on the karna UI. |
| `posted` | Whether the finding made it onto GitHub. `false` = dropped during validation or by GitHub. |
| `skip_reason` | When `posted=false`, why (e.g. `"line 412 not in diff"`, `"empty body or path"`). NULL when posted. |

**State** (`pr_reviews` table):

| Column | Purpose |
|---|---|
| `(repo, head_sha)` UNIQUE | Dedupe; resync triggers on force-push but not on same SHA |
| `status` | `pending` (enqueued by webhook) → `running` (claimed by agent poll) → `completed` / `failed` / `skipped` |
| `reviewer_agent_id` | Which agent profile ran the review (FK SET NULL on profile delete) |
| `cost_usd` | Theoretical quota burn from CLI output (subscription doesn't charge this — see "Agent Profiles" note on cost tracking) |
| `comments_posted` | Number of inline review comments that landed on the PR via the structured-findings flow. Zero when the review was body-only (clean diff, parse failure fallback, or POST fallback) |

`db.claim_pending_pr_review()` uses `FOR UPDATE SKIP LOCKED` on the `pending → running` transition so multiple agent replicas can poll the same queue without stepping on each other.

**Progress comment** — when a review starts, the agent posts a "🤖 Karna review in progress" comment on the PR (via `gh api .../issues/{n}/comments`) so the author sees instant feedback that the webhook fired and the agent picked it up. When the review finishes:
- Success: the comment is edited to "🤖 Karna review complete — see the review below."
- Failure: edited to "🤖 Karna review failed" with a truncated error message in a code block.

Comment ID is held in-memory during the run (no DB column); if the agent crashes mid-review the comment stays as "in progress" until the next force-push triggers a new review.

**Live progress streaming** — CLI tool calls and assistant text are streamed into `pr_review_logs` via `spawn_review_log_consumer` (mirrors the task-side `spawn_log_consumer`). The UI polls `GET /api/repos/{id}/reviews/{review_id}/logs` every 2 seconds while the review is `running` / `pending`, frozen on terminal status. A short Redis TTL (5s) keeps the cache from masking new entries while still cutting Postgres traffic.

**Bot-comment filter** — applied at the **top** of **both** GitHub webhook handlers (karna-api and karna-agent), before any branch / agent-vs-human routing. Any `issue_comment`, `pull_request_review_comment`, or `pull_request_review` event whose user's `type == "Bot"` is dropped immediately. This covers Vercel preview deploys, GitHub Actions status updates, dependabot, renovate, and Karna's own progress-comment edits on both agent PRs and human PRs. Bot-authored PRs (e.g. renovate dependency updates) still get auto-reviewed normally — the filter is on commenters/reviewers, not PR authors.

**Manual re-registration** — `POST /api/repos/{id}/webhook` flips `webhook_status` to `not_registered`; the reconciler picks it up on its next poll cycle (typically within seconds). The repo card surfaces a "Re-register" button on the webhook status row when sync is enabled but the hook isn't live. The webhook URL match is now case-insensitive on scheme + host and trailing-slash insensitive on path — manually-configured hooks that differed only cosmetically used to be missed.

**Frontend:**
- Repo detail page ([app/(dashboard)/repos/[id]/page.tsx](frontend/app/(dashboard)/repos/[id]/page.tsx)) has the "Auto-review PRs" toggle, the "Review agent" dropdown, the webhook re-register button (shows whenever `sync_issues || review_prs` is on), and a **PR reviews** section listing the last 10 reviews with status, author, head SHA, cost, and a link to the PR. Auto-refresh every 3s while any review is `running`, every 15s otherwise.
- [review-log-modal.tsx](frontend/components/agent/review-log-modal.tsx) — click a review row to see live logs (timestamps, tool calls, assistant text, errors). Renders a **Findings** section above the activity log: posted-inline findings (green badge) and skipped findings (amber badge with the `skip_reason`) so reviewers can see what got dropped. Each finding shows a severity badge (red `Sev: High`, amber `Sev: Medium`, sky-blue `Sev: Low`) and the list is sorted high → low so the things that actually need attention show up first. High-severity rows get a brighter container so they stand out even after merge. Polls logs every 2s and findings every 3s while live.
- Repo card ([repo-card.tsx](frontend/components/agent/repo-card.tsx)) shows a purple `reviews` badge when enabled (amber with `no hook` when the webhook isn't live).

**Cost model — important:** `cost_usd` on `pr_reviews` is the same theoretical-API-equivalent figure that the Claude CLI emits (`total_cost_usd` in the stream JSON). With a subscription, you aren't billed it — but it's a useful proxy for quota burn. To minimize quota impact, set `review_agent_id` to a cheap-model profile (haiku, gpt-5.4-mini) per repo.

**Key files:**
- [agent/src/reviewer.rs](agent/src/reviewer.rs) — `enqueue_review` (webhook entry), `run_pending_reviews` (poll-loop drain), `process_review_row` (per-row CLI work), `parse_findings`, `DiffIndex::parse`, `validate_findings`, `post_structured_review`, system prompt, progress comment + log streaming
- [agent/src/main.rs](agent/src/main.rs) — Poll loop calls `reviewer::run_pending_reviews` every tick
- [agent/src/api/mod.rs](agent/src/api/mod.rs) — `handle_pr_review_trigger` (enqueue arm) + bot-comment filter + `trigger_webhook_register` debug endpoint
- [api/src/routes/webhooks.rs](api/src/routes/webhooks.rs) — `handle_pr_review_trigger` (the production webhook entry — enqueues a `pending` row) + bot-comment filter
- [agent/src/git/github.rs](agent/src/git/github.rs) — `webhook_urls_equivalent` (forgiving match), `WebhookEnsureResult`
- [agent/src/onboarding.rs](agent/src/onboarding.rs) — `register_webhook` returns `Result<String, String>` (matched URL or error)
- [shared/src/db.rs](shared/src/db.rs) — `enqueue_pr_review` (race-safe insert), `claim_pending_pr_review` (FOR UPDATE SKIP LOCKED claim), `complete_pr_review`, `insert_pr_review_log`, `get_pr_review_logs`, `insert_pr_review_finding`, `get_pr_review_findings`, `update_repo_review_config`
- [api/src/routes/repos.rs](api/src/routes/repos.rs) — `list_reviews`, `review_logs`, `review_findings`, `trigger_webhook_register`

## Redis Queue Protocol

```
# Claim task (atomic, one worker wins)
SET task_lock:{task_id} {worker_id} NX EX 1800

# Heartbeat (extend while working)
EXPIRE task_lock:{task_id} 1800

# Release (on completion or failure)
DEL task_lock:{task_id}
```

## Redis Cache (read-through)

To minimize Postgres egress, all GET endpoints in the Rust API are cached in Redis with a 10-minute TTL. Cache invalidation lives **inside `karna_shared::db::Database`** — every write method (`update_status`, `set_plan`, `insert_log`, `upsert_repo_profile`, …) busts the relevant cache keys automatically. Because both `karna-api` and `karna-agent` share the same `Database`, agent-side writes invalidate just like API-side writes.

```
cache:tasks:list:{user_id}              # GET /api/tasks
cache:tasks:logs:{task_id}              # GET /api/tasks/{id}/logs
cache:schedules:list:{user_id}          # GET /api/schedules
cache:schedules:runs:{schedule_id}      # GET /api/schedules/{id}/runs
cache:schedules:run_logs:{run_id}       # GET /api/schedules/{id}/runs/{run_id}/logs
cache:repos:list                        # GET /api/repos
cache:agents:list                       # GET /api/agents
cache:policies:list                     # GET /api/policies
cache:reviews:repo:{repo_id}            # GET /api/repos/{id}/reviews   (60s TTL)
cache:reviews:logs:{review_id}          # GET /api/repos/{id}/reviews/{review_id}/logs (5s TTL — live)
cache:reviews:findings:{review_id}      # GET /api/repos/{id}/reviews/{review_id}/findings (30s TTL)
cache:config                            # GET /api/config (also busted by repo writes)
```

Pattern invalidation (`cache:tasks:list:*`) is used for cross-user list caches since default-system-user rows are visible to everyone. Cache failures never fail the request — read-through falls back to Postgres, writes proceed without busting.

**Wiring:** `Database::connect(url).await?.with_redis(redis.clone())` — without `with_redis`, the cache layer is silently disabled (useful for tests).

**Key files:**
- `shared/src/cache.rs` — `get_or_set`, `invalidate`, `invalidate_pattern`, key builders
- `shared/src/db.rs` — `bust_tasks`/`bust_schedules`/`bust_repos`/`bust_task_logs`/`bust_schedule_run` called from every write method
- `api/src/routes/{tasks,schedules,repos,config}.rs` — read-through wrappers

## Environment Variables

All secrets live in `.env` (gitignored). User config lives in `config.yaml` (gitignored). Template lives in `config.example.yaml` (tracked).

| Variable | Required | Purpose |
|----------|----------|---------|
| CLAUDE_CODE_OAUTH_TOKEN | Yes (cli: claude) | Claude Code CLI (OAuth token from `claude setup-token`) |
| OPENROUTER_API_KEY | Yes (cli: opencode) | Pay-per-token access to any model via OpenRouter. Get one at https://openrouter.ai/settings/keys |
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
| KARNA_SHARED_WORKSPACE | No | `true` = all signed-in users see/edit all tasks. For small teams; pair with `SIGNUP_DISABLED=true` |
| GITHUB_WEBHOOK_SECRET | No | HMAC-SHA256 secret for webhook signature verification |
| LINEAR_WEBHOOK_SECRET | No | HMAC-SHA256 secret for `/webhooks/linear` |
| LINEAR_API_KEY | No | Posts PR backlink comments onto Linear issues |
| CLICKUP_WEBHOOK_SECRET | No | HMAC-SHA256 secret for `/webhooks/clickup` |
| CLICKUP_API_TOKEN | No | Posts PR backlink comments onto ClickUp tasks |
| AUTH_GOOGLE_ID | No | Google OAuth client ID; enables "Continue with Google" on login |
| AUTH_GOOGLE_SECRET | No | Google OAuth client secret (required with AUTH_GOOGLE_ID) |
| AUTH_ALLOWED_EMAIL_DOMAINS | No | Comma-separated email-domain allowlist for Google sign-in (e.g. `company.com,partner.com`) |

### Google OAuth (Auth.js)

Optional second provider alongside the default Credentials (email/password) flow. Set `AUTH_GOOGLE_ID` + `AUTH_GOOGLE_SECRET` and the login page renders a "Continue with Google" button. `AUTH_ALLOWED_EMAIL_DOMAINS` (comma-separated) gates which domains can sign in; leave empty to allow any Google account (not recommended).

- Provider wiring lives in [frontend/auth.ts](frontend/auth.ts) (conditionally pushed into the providers array, plus a `signIn` callback that enforces the domain allowlist)
- Login UI button in [frontend/app/login/login-form.tsx](frontend/app/login/login-form.tsx); page reads env via [frontend/app/login/page.tsx](frontend/app/login/page.tsx) and passes a `googleEnabled` prop
- Callback URL to register in Google Cloud Console: `{AUTH_URL}/api/auth/callback/google`
- First-time Google sign-in auto-creates a `users` row via `@auth/pg-adapter` (with no password — the Credentials provider rejects passwordless accounts, so Google-only users stay Google-only)
- `allowDangerousEmailAccountLinking: true` lets an existing credentials account log in via Google with the same email (Google verifies email so this is safe)

**Helm**: set `auth.google.enabled: true` and either inline `clientId`/`clientSecret` or reference `auth.google.existingSecret`. `auth.google.allowedEmailDomains` becomes `AUTH_ALLOWED_EMAIL_DOMAINS`. NOTES.txt prints the callback URL post-install.

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
    opencode:
      models: [openrouter/moonshotai/kimi-k2.6, openrouter/deepseek/deepseek-v3]
      default_model: openrouter/moonshotai/kimi-k2.6
```

| Backend | Binary | Auth | Models | MCP Support | Project Instructions |
|---------|--------|------|--------|-------------|---------------------|
| `claude` | `claude` | `~/.claude` (volume mount) | opus, sonnet, haiku | Yes (`--mcp-config`) | CLAUDE.md |
| `codex` | `codex` | `~/.codex` (volume mount) | gpt-5.4, gpt-5.4-mini, gpt-5.3-codex | No | AGENTS.md |
| `opencode` | `opencode` | `OPENROUTER_API_KEY` env (pay-per-token, any provider via OpenRouter) | Anything on openrouter.ai (Kimi K2, DeepSeek, Qwen, GLM, etc.) | Yes — Karna writes `~/.config/opencode/opencode.json` at startup, translated from `mcp_servers` in `config.yaml` | AGENTS.md |

**Per-task columns:** `agent_tasks.cli` + `agent_tasks.model` (both nullable, default from config). Subtasks inherit parent's cli/model.

**Dispatch flow:** `cli::run(task.cli, opts)` → `claude::run()` or `codex::run()` or `opencode::run()`

**AGENTS.md symlink:** Automatically created as `AGENTS.md → CLAUDE.md` in every repo/worktree so Codex and opencode can read the same project instructions. Created by `workspace::ensure_agents_md_symlink()` after clone and worktree creation.

**opencode model strings:** Passed verbatim to `opencode -m`. The OpenRouter convention is `openrouter/<owner>/<model>` (e.g. `openrouter/moonshotai/kimi-k2.6`). To pin a specific OpenRouter sub-provider, drop an `opencode.json` into the repo (opencode merges project over global).

**opencode MCP:** Karna translates the global `mcp_servers` list in `config.yaml` from the Claude shape (`{"mcpServers": {"name": {"command": "...", "args": [...], "env": {...}}}}`) into opencode's shape (`{"$schema": "...", "mcp": {"name": {"type": "local", "command": ["..."], "environment": {...}, "enabled": true}}}`) and writes it to `~/.config/opencode/opencode.json` on every startup + config hot-reload (`Config::write_opencode_global_config()` in [agent/src/config.rs](agent/src/config.rs)). Remote servers (`type: sse|http|remote`) become `type: remote` with `headers` instead of `environment`. The per-task `mcp_config_json` arg (which the Claude backend uses with `--mcp-config`) is ignored by the opencode runner — repo `.mcp.json` files are still in Claude format and don't apply to opencode; if you want per-repo MCP for opencode, commit a project-level `opencode.json` to the repo.

**Key files:**
- `agent/src/cli.rs` — Common `CliOptions`/`CliResult` types + dispatch
- `agent/src/claude/mod.rs` — Claude Code CLI (`-p --dangerously-skip-permissions --output-format json`)
- `agent/src/codex/mod.rs` — Codex CLI (`--full-auto --quiet`)
- `agent/src/opencode/mod.rs` — opencode CLI (`run --format json --dangerously-skip-permissions -m <provider/model>`)
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

## Kubernetes / Helm

Production-grade Helm chart in `charts/karna/`. Packages all services for Kubernetes deployment with Bitnami subcharts for PostgreSQL and Redis.

### Quick Start

```bash
# Add Bitnami dependency repo
helm dependency update charts/karna

# Install with minimal config
helm install karna charts/karna \
  --set github.token=ghp_... \
  --set claude.oauthToken=... \
  --set config.repos[0].repo=owner/my-app \
  --set config.repos[0].branch=main

# Production install
helm install karna charts/karna \
  -f charts/karna/values.production.yaml \
  -f my-values.yaml
```

### Chart Structure

```
charts/karna/
├── Chart.yaml                    # Metadata + Bitnami PostgreSQL/Redis dependencies
├── values.yaml                   # Default configuration
├── values.production.yaml        # Production overlay (higher replicas, stricter limits)
├── files/
│   ├── migrations/               # SQL migrations + runner (synced from migrations/)
│   └── skills/                   # Built-in skill files (synced from skills/)
└── templates/
    ├── _helpers.tpl              # Reusable helpers (fullname, labels, DB/Redis URLs)
    ├── NOTES.txt                 # Post-install instructions
    ├── configmap.yaml            # config.yaml from values
    ├── configmap-migrations.yaml # Migration files (pre-install hook)
    ├── configmap-skills.yaml     # Skill files for agent
    ├── secret.yaml               # All credentials (supports existingSecret)
    ├── migration-job.yaml        # Pre-install/upgrade Job
    ├── serviceaccount.yaml
    ├── pdb.yaml                  # Pod disruption budgets
    ├── ingress.yaml              # Main + agent webhook + code-server
    ├── api/                      # Deployment, Service, HPA
    ├── agent/                    # Deployment, Service, PVC (workspace)
    ├── frontend/                 # Deployment, Service
    ├── code-server/              # Deployment, Service, PVC (optional)
    └── tests/                    # Helm test pod
```

### Key Differences from Docker Compose

| Docker Compose | Helm Chart |
|---|---|
| autoheal container | Kubernetes liveness/readiness probes + restart policies |
| Cloudflare tunnel | Kubernetes Ingress (nginx, traefik, ALB) |
| `docker compose --scale agent=N` | `agent.replicaCount: N` in values |
| `.env` file | Kubernetes Secrets (or `existingSecret` refs) |
| Named volumes | PersistentVolumeClaims |
| `migrate` service | Pre-install/upgrade Job with hook |

### External Database/Redis

Set `postgresql.enabled: false` / `redis.enabled: false` and configure `postgresql.external.*` / `redis.external.*` to use managed services.

### Scaling Agents

Multiple agent replicas require `ReadWriteMany` (RWX) storage for the workspace PVC. Set `agent.workspace.accessMode: ReadWriteMany` and use an RWX-capable storage class (EFS, NFS, CephFS).

### File Sync

`charts/karna/files/migrations/` and `charts/karna/files/skills/` mirror the repo-root `migrations/` and `skills/` dirs (the source of truth). Helm only bundles files inside the chart dir, so the chart copies must exist for `helm package` to pick them up.

- The release workflow ([.github/workflows/release.yml](.github/workflows/release.yml)) `rsync`s both dirs before `helm package` — releases always ship from the canonical source even if the in-repo copies have drifted.
- CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) fails the `helm` job when the dirs disagree, so PRs that touch migrations or skills can't merge without updating the chart copies.
- Locally, after editing migrations or skills: `rsync -a --delete migrations/ charts/karna/files/migrations/` and the same for `skills/`.

### Extending the Agent Pod (Custom CLIs + Secrets)

Skills can require custom binaries (e.g. `terraform`, `aws`, vendor CLIs) and tokens that karna doesn't know about. Four escape hatches on `agent.*` keep this out-of-tree — no chart fork, no rebuilt `karna-agent` image:

| Values key | Type | Purpose |
|---|---|---|
| `agent.extraEnv` | `[]corev1.EnvVar` | Append env vars (raw `value:` or `valueFrom:`). Templated via `tpl`, so Helm refs work. |
| `agent.extraEnvFrom` | `[]corev1.EnvFromSource` | Bulk-load every key of a `Secret`/`ConfigMap` as env. |
| `agent.extraInitContainers` | `[]corev1.Container` | Run before agent starts. Common pattern: copy a vendor CLI into a shared emptyDir. |
| `agent.extraVolumes` / `agent.extraVolumeMounts` | volumes / mounts | Pair an emptyDir (or configMap/secret) with a mount on PATH (e.g. `/home/agent/.local/bin`). |

**Typical custom-CLI install pattern:** init container copies the binary from a vendor image into an `emptyDir` volume; the agent container mounts the same emptyDir somewhere on PATH. Tokens for the CLI come in via `extraEnv` pointing at a user-managed `Secret` (or External Secrets Operator / Vault / SOPS — the chart only references the Secret name, never the value).

Wiring lives in [charts/karna/templates/agent/deployment.yaml](charts/karna/templates/agent/deployment.yaml) (each block guarded by `with`, so empty lists render nothing) and defaults in [charts/karna/values.yaml](charts/karna/values.yaml).

## Rules

- Tailwind classes only, no vanilla CSS
- No shadcn component modifications — use className overrides
- API routes validate auth via `auth()` before any DB query
- Agent backend uses service-level DB access (no row-level security)
- One task at a time per worker (configurable via max_concurrent_tasks)
