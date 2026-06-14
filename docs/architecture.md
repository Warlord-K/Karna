# Karna Architecture (Current Implementation)

This document describes Karna as it exists in the current codebase, including partial behavior and known constraints.

## System Components

| Component | Path | Responsibility |
|---|---|---|
| Agent worker (Rust) | `agent/src` | Polls actionable tasks, runs planning/implementation/orchestrator flows, manages git worktrees, opens PRs, writes logs |
| API server (Rust/Axum) | `api/src` | Authenticated REST API, task CRUD, orchestrator task creation, SSE log stream, webhook ingestion |
| Frontend (Next.js) | `frontend` | Board UI, task detail view, chat UI, SSE + polling fallback, task creation forms |
| Shared crate (Rust) | `shared/src` | DB queries, task/log/domain models used by both agent and API |
| PostgreSQL | migrations in `migrations` | Source of truth for tasks, logs, repo profiles, schedules, agent profiles, auth tables |
| Redis | queue + cache | Worker locks/heartbeats/trigger keys and API response caching |
| Optional mem0 | `docker-compose.yml`, `charts/karna/templates/mem0` | External memory store for prompt injection + write-back |

## High-Level Runtime Flow

1. UI or webhook creates/updates rows in `agent_tasks`.
2. Agent loop (`agent/src/agent/mod.rs`) calls `next_actionable_task()`, acquires a Redis lock, then dispatches by task `kind` and `status`.
3. CLI backend streams events -> `StreamEvent` -> `spawn_log_consumer()` -> `agent_logs`.
4. API exposes logs via:
   - `GET /api/tasks/{id}/logs` (polling)
   - `GET /api/tasks/{id}/logs/stream` (SSE)
5. Frontend uses SSE when available and falls back to polling when stream connection fails.

## Task Model and Lifecycle

### Statuses

`TaskStatus` in `shared/src/models.rs`:

- `todo`
- `planning`
- `plan_review`
- `in_progress`
- `review`
- `done`
- `failed`
- `cancelled`

Typical code-task progression:

`todo -> planning -> plan_review -> in_progress -> review -> done`

Failure/exit paths:

- any stage error -> `failed` (`set_error`)
- manual/Slack cancel -> `cancelled`
- merged PR webhook (`pull_request.closed` + merged=true) -> `done`

Feedback re-entry paths:

- `plan_review` + feedback -> `planning`
- `review` + feedback -> `in_progress`

### Task Kind and Dispatch

`kind` is persisted on `agent_tasks` (`code | doc | research | ops`).

Dispatch in `poll_once()`:

- `kind=code` (or unknown): planner/implementer flow
- `kind!=code`: generic flow (`flow::run_generic`)
  - `kind=ops` + `orchestrator` config -> orchestrator loop
  - otherwise -> standard non-code generic run

`output_target` controls non-code artifact handling (`none`, `notification`, `slack_message`, `linear_comment`, `linear_doc`, `pr`).

## Stage-Based Runtime Resolution

Stage enum in `agent/src/agent/mod.rs`:

- `Plan`
- `Implement`
- `Review` (self-review stage)

Runtime resolution (`resolve_runtime`) precedence is:

1. Stage-specific profile (`planner_agent_id`, `implementer_agent_id`, `reviewer_agent_id`)
2. Task-level `cli` / `model`
3. Task-level `assigned_agent_id` profile
4. Config defaults (`agent.backends`)

System prompt content is merged as:

- global instructions file (`agent.instructions`)
- optional profile `system_prompt_addendum`

## Code Task Flow

### Plan Stage

Planner (`agent/src/agent/planner.rs`):

- sets status `planning`
- discovers repos to inspect
- loads global + repo skills, merges repo `.mcp.json` with configured MCP
- injects memory snippets when enabled
- runs backend with read-only tool allowlist: `Read,Glob,Grep,Bash`
- writes plan to `plan_content` and sets status `plan_review`
- parses optional `<!-- subtasks -->` block (consumed later by API endpoint)

### Implement Stage

Implementer (`agent/src/agent/implementer.rs`):

- creates per-repo git worktrees
- runs selected backend with write-capable tools
- runs bounded self-review loop (below)
- commits/pushes and opens PR(s)
- sets task to `review` when PR exists

### Self-Review Loop (Implemented)

Karna now has an internal implement <-> self-review loop:

- reviewer stage runs over current git diff (`self_review::review_diff`)
- required output sections: `===VERDICT===`, `===FLAGS===`, `===CHANGES===`
- verdict `CHANGES` triggers another implementer run (session resume when possible)
- bounded by `agent.max_review_rounds`
- if max rounds reached, Karna ships current changes (logs warning)

Important current behavior: if self-review output is malformed/unactionable, parser defaults to `APPROVE` rather than blocking indefinitely.

### PR and Review Gate Behavior

- Opening PR transitions task to `review`.
- Approving review in Slack does **not** mark task `done`; it only logs approval and tells operator to merge.
- Task reaches `done` when merge webhook arrives (or other explicit status update).

## Multi-Backend CLI Layer and Stream Mapping

Dispatcher: `agent/src/cli.rs`.

Backends:

- `claude` (`agent/src/claude/mod.rs`)
- `codex` (`agent/src/codex/mod.rs`)
- `cursor` (`agent/src/cursor/mod.rs`)
- `grok` (`agent/src/grok/mod.rs`)
- `opencode` (`agent/src/opencode/mod.rs`)

Mapped stream events:

- `ToolUse`
- `AssistantText`
- `Error`

`spawn_log_consumer()` writes them into `agent_logs` as `tool`, `output`, or `error`.

Backend nuances as implemented:

- `claude`: native tool_use blocks and cost tracking (`total_cost_usd`)
- `codex`: JSON events mapped to tool/message events; cost currently recorded as `0.0`
- `cursor`: Claude-like stream-json envelope parsing; no image input support
- `grok`: text delta streaming only; no discrete tool call events emitted
- `opencode`: best-effort JSON event parsing; MCP read from global opencode config

## Orchestrator Tasks

Orchestrator entrypoint:

- task `kind=ops`
- non-null `orchestrator` JSON config (`OrchestratorConfig`)

Action contract parser (`agent/src/agent/actions.rs`) expects:

```text
<!-- actions
[ ...json actions... ]
actions -->
```

Supported action types:

- `reply`
- `run`
- `defer`
- `subtask`
- `escalate`
- `close`

Guardrails enforced in code:

- `max_actions_per_turn`
- `max_subtasks`
- `allowed_tools` matching (`exact`, `server/*`, or server prefix)
- `max_turns` and optional `deadline`

### Important current limitation: `run` is two-turn

`run` does not execute an MCP call immediately inside `execute_actions()`.  
Current v1 behavior records an approved instruction into task feedback so the **next orchestrator turn** performs the MCP call inline with context.

### Deferral and Resume

- `defer` sets `not_before = now + duration` (`s/m/h/d` units)
- poller only selects tasks with `not_before IS NULL OR <= now()`
- deferred note is stored into feedback for resumed turn context

### External Thread Replies

If a Slack message lands in a mapped task thread:

- allowlisted operator users can always steer tasks through command/feedback surface
- non-allowlisted users are accepted only when `orchestrator.accepts_external_replies=true`

This is the current watched-thread mechanism used by orchestrator tasks.

## Chat UI as Orchestrator Tasks

Frontend chat (`/chat`) creates tasks through `POST /api/orchestrator-tasks` with:

- `kind=ops`
- `source='chat'`
- description = first message
- optional repo scope

Chat-specific behavior in API:

- if `source='chat'` and no `allowed_tools` provided, it defaults to configured MCP server names
- sets `accepts_external_replies=false`

Execution behavior:

- if repo scope is set, orchestrator runs in cloned repo directory
- tool allowlist remains read-only (`Read,Glob,Grep,Bash`) for orchestrator turn execution
- chat flow does not directly run git/PR pipeline

## Memory (mem0)

Memory client: `agent/src/memory/mod.rs`.

Namespaces:

- `repo:<owner/repo>`
- `agent:<profile_slug>`
- `user:<uuid>`

Injection:

- planner and implementer inject repo + agent memory snippets
- generic/orchestrator flow also includes user namespace

Write-back:

- poll loop scans done tasks not yet marked with `memory` completion log
- writes concise summary to repo + agent + user namespaces
- failures are best-effort and non-blocking

## Slack Control Plane

Slack module: `agent/src/slack/mod.rs` (Socket Mode).

Required for inbound control:

- `slack.enabled=true`
- `SLACK_BOT_TOKEN` + `SLACK_APP_TOKEN`

Operator command surface:

- explicit mention (`app_mention`, `@karna`, or direct `<@...>` prefix)
- user must be in `allowed_user_ids`
- mapped to Karna user via `slack.user_map`

Implemented commands:

- `implement <LINEAR-123>`
- `new [owner/repo] <title> - <description>`
- `status` / `list`
- `cancel <task-id|task-number|title-fragment>`

Interactive gates:

- Plan: approve / request changes / cancel
- Review: approve / request changes / cancel

Current nuance: review approve does not auto-complete task; merge webhook still drives `done`.

## Real-Time Delivery

Backend:

- SSE endpoint: `GET /api/tasks/{id}/logs/stream`
- polls DB every second server-side, emits `event: log` payloads

Frontend:

- tries EventSource first
- on SSE failure or no EventSource support, falls back to polling `GET /api/tasks/{id}/logs` every 3s

## Where Things Live

### Agent Crate (`agent/src`)

- `agent/mod.rs`: poll loop dispatch, lock handling, stage runtime resolution
- `agent/planner.rs`: plan stage
- `agent/implementer.rs`: implement + PR + feedback apply
- `agent/self_review.rs`: review contract parser + reviewer stage runner
- `agent/flow.rs`: non-code + orchestrator flow
- `agent/actions.rs`: orchestrator action parsing/execution/guardrails
- `cli.rs` + backend modules (`claude`, `codex`, `cursor`, `grok`, `opencode`): CLI adapters
- `memory/mod.rs`: mem0 integration
- `slack/mod.rs`: Slack Socket Mode control plane
- `preflight.rs`: startup checks for configured backends
- `config.rs`: config loading + env resolution + MCP translation

### API Crate (`api/src`)

- `main.rs`: router wiring (REST + SSE + webhooks)
- `routes/tasks.rs`: tasks/chats/orchestrator endpoints, comments, SSE logs
- `routes/webhooks.rs`: GitHub/Linear/ClickUp webhook handling
- `routes/agents.rs`, `routes/repos.rs`, `routes/schedules.rs`, etc.

### Shared Crate (`shared/src`)

- `models.rs`: enums and structs (`AgentTask`, `TaskStatus`, `TaskKind`, etc.)
- `db.rs`: SQL access layer used by agent + API

