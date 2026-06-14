-- Soft task kinds and generic output targets.
-- Keep `code` as the default so existing code-task behavior is unchanged.

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'code';

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS output_target TEXT;

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS output_ref TEXT;
