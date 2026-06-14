-- Orchestrator task controls: deferred pickup + per-task constraints.

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS not_before TIMESTAMPTZ;

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS orchestrator JSONB;
