-- Task assignment to humans + external task source (Linear / ClickUp ingest)
--
-- assignee_user_id NULL  → the agent picks up the task (existing behavior)
-- assignee_user_id SET   → assigned to a human; agent must skip it
--
-- external_source / external_id / external_url track tasks that originate from
-- Linear or ClickUp so we can post PR backlinks and avoid duplicate ingest.

ALTER TABLE agent_tasks
  ADD COLUMN assignee_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  ADD COLUMN external_source  TEXT,
  ADD COLUMN external_id      TEXT,
  ADD COLUMN external_url     TEXT;

-- Filter agent's actionable-task query without a full table scan
CREATE INDEX IF NOT EXISTS idx_agent_tasks_assignee
  ON agent_tasks (assignee_user_id)
  WHERE assignee_user_id IS NOT NULL;

-- Fast dedupe lookup when a webhook arrives
CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_tasks_external
  ON agent_tasks (external_source, external_id)
  WHERE external_source IS NOT NULL AND external_id IS NOT NULL;
