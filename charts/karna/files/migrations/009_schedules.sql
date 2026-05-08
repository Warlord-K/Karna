-- Schedules: recurring and one-shot automated agent runs
CREATE TABLE IF NOT EXISTS schedules (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

  name TEXT NOT NULL,
  prompt TEXT NOT NULL,
  repos TEXT,            -- comma-separated "owner/repo" list, NULL = all configured repos

  -- Timing: exactly one of cron_expression or run_at should be set
  cron_expression TEXT,  -- standard 5-field cron (e.g. "0 */4 * * *")
  run_at TIMESTAMPTZ,    -- one-shot: run at this time, then auto-disable

  -- Execution config
  skills TEXT[] DEFAULT '{}',
  mcp_servers TEXT[] DEFAULT '{}',
  max_open_tasks INTEGER NOT NULL DEFAULT 3,
  task_prefix TEXT,      -- prefix for created tasks (e.g. "BUG", "FEA")
  priority TEXT NOT NULL DEFAULT 'medium'
    CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
  cli TEXT,              -- NULL = config default
  model TEXT,            -- NULL = backend default

  enabled BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_schedules_user_id ON schedules(user_id);
CREATE INDEX IF NOT EXISTS idx_schedules_enabled ON schedules(enabled) WHERE enabled = true;

CREATE TRIGGER update_schedules_updated_at
  BEFORE UPDATE ON schedules
  FOR EACH ROW
  EXECUTE FUNCTION update_updated_at();

-- Scheduled runs: one record per schedule execution
CREATE TABLE IF NOT EXISTS scheduled_runs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  schedule_id UUID NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,

  status TEXT NOT NULL DEFAULT 'running'
    CHECK (status IN ('running', 'completed', 'failed')),

  started_at TIMESTAMPTZ DEFAULT NOW(),
  completed_at TIMESTAMPTZ,
  summary_markdown TEXT,        -- LLM output: what was found/decided
  tasks_created UUID[] DEFAULT '{}',
  cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,

  created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_scheduled_runs_schedule_id ON scheduled_runs(schedule_id);
CREATE INDEX IF NOT EXISTS idx_scheduled_runs_started_at ON scheduled_runs(started_at);

-- Scheduled run logs: append-only (mirrors agent_logs pattern)
CREATE TABLE IF NOT EXISTS scheduled_run_logs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  run_id UUID NOT NULL REFERENCES scheduled_runs(id) ON DELETE CASCADE,

  level TEXT NOT NULL DEFAULT 'info'
    CHECK (level IN ('info', 'error', 'warn', 'debug')),
  message TEXT NOT NULL,

  created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_scheduled_run_logs_run_id ON scheduled_run_logs(run_id);
