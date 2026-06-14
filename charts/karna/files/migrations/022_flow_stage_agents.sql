-- Per-stage agent profile selection for the multi-agent flow.
--
-- A task's pipeline (scope/plan → implement ↔ self-review) can run each stage
-- on a different tool/model identity. These columns reference agent_profiles,
-- one per stage. Resolution precedence (see agent::resolve_runtime):
--   stage column → task.cli/model override → task.assigned_agent_id → config default
-- NULL everywhere falls back to the config default, preserving existing behavior.

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS planner_agent_id UUID
    REFERENCES agent_profiles(id) ON DELETE SET NULL;

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS implementer_agent_id UUID
    REFERENCES agent_profiles(id) ON DELETE SET NULL;

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS reviewer_agent_id UUID
    REFERENCES agent_profiles(id) ON DELETE SET NULL;
