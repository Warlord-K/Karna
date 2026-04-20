-- Repo profiles: auto-discovered summaries of configured repositories
CREATE TABLE IF NOT EXISTS repo_profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

  -- Repo identity
  repo TEXT NOT NULL UNIQUE,        -- "owner/repo" format
  branch TEXT NOT NULL DEFAULT 'main',

  -- Onboarding status
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'onboarding', 'ready', 'failed', 'stale')),

  -- Profile data (from CLI exploration)
  summary TEXT,                     -- Human-readable markdown summary
  profile_json JSONB,               -- Structured data (language, framework, commands, etc.)

  -- Metadata
  last_onboarded_at TIMESTAMPTZ,
  last_commit_sha TEXT,             -- Track when profile goes stale
  error_message TEXT,
  cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,

  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_repo_profiles_repo ON repo_profiles(repo);
CREATE INDEX IF NOT EXISTS idx_repo_profiles_status ON repo_profiles(status);

CREATE TRIGGER update_repo_profiles_updated_at
  BEFORE UPDATE ON repo_profiles
  FOR EACH ROW
  EXECUTE FUNCTION update_updated_at();
