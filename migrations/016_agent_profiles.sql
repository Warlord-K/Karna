-- Agent profiles as pseudo-users.
--
-- Today the agent is anonymous: assignee_user_id NULL means "any agent picks it up."
-- This migration introduces named agent identities so a task can be assigned to a
-- specific agent (e.g. "Sonnet" or "Codex GPT-5.4"), and the agent worker picks
-- up only tasks that match its catalog. Profiles are auto-seeded from
-- config.yaml on agent startup; users can rename / add / pause them.
--
-- Assignment semantics on agent_tasks:
--   assignee_user_id SET    → human owns it, agent skips (existing behavior)
--   assigned_agent_id SET   → only that agent picks it up
--   both NULL               → any agent picks it up (default, existing behavior)

CREATE TABLE IF NOT EXISTS agent_profiles (
  id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  slug                     TEXT NOT NULL UNIQUE,
  name                     TEXT NOT NULL,
  avatar_emoji             TEXT NOT NULL DEFAULT '🤖',
  cli                      TEXT NOT NULL,
  model                    TEXT NOT NULL,
  system_prompt_addendum   TEXT,
  -- NULL = active. Set = paused with a human-readable reason
  -- (e.g. "rate limited", "manually paused"). When set, the agent worker
  -- will not claim tasks assigned to this profile.
  paused_reason            TEXT,
  is_default               BOOLEAN NOT NULL DEFAULT FALSE,
  created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Only one default profile at a time. NULL is_default doesn't conflict
-- because we coerce false rows via a partial index.
CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_profiles_default
  ON agent_profiles (is_default)
  WHERE is_default = TRUE;

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS assigned_agent_id UUID
    REFERENCES agent_profiles(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_agent_tasks_assigned_agent
  ON agent_tasks (assigned_agent_id)
  WHERE assigned_agent_id IS NOT NULL;
