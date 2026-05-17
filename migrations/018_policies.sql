-- Policies: path-based guardrails surfaced on the plan_review tab.
--
-- A policy says: "when a task's plan touches files matching `path_glob` in
-- `repo_pattern`, surface this message." It's advisory (severity = 'warn') by
-- default — the reviewer sees a banner but can still approve. 'block' severity
-- is reserved for future use (the UI will dim the approve button); the agent
-- still produces the plan either way.
--
-- Typical examples:
--   ("migrations/**", "warn", "Schema change — verify rollback plan")
--   ("auth/**",       "warn", "Auth path — check session/permission impact")
--   ("**/*.env*",     "block", "Secrets path — review carefully")

CREATE TABLE IF NOT EXISTS policies (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name            TEXT NOT NULL,
  -- Repo glob: "owner/repo" exact, "owner/*" prefix, or "*" all repos.
  repo_pattern    TEXT NOT NULL DEFAULT '*',
  -- Glob against file paths mentioned in plan_content (supports ** and *).
  path_glob       TEXT NOT NULL,
  message         TEXT NOT NULL,
  severity        TEXT NOT NULL DEFAULT 'warn'
    CHECK (severity IN ('warn', 'block')),
  enabled         BOOLEAN NOT NULL DEFAULT TRUE,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_policies_enabled
  ON policies (enabled)
  WHERE enabled = TRUE;

-- Matched policies are stored on the task itself so the UI can render the
-- banner from a single GET /api/tasks call. Shape:
-- [{policy_id, name, severity, message, paths: [matched_path, ...]}]
ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS policy_matches JSONB;
