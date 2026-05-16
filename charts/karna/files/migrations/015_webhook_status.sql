-- Track GitHub webhook registration status per repo so the UI can surface
-- when issue sync is enabled but no webhook is actually live.
ALTER TABLE repo_profiles
  ADD COLUMN IF NOT EXISTS webhook_status TEXT NOT NULL DEFAULT 'not_registered'
    CHECK (webhook_status IN ('not_registered', 'registered', 'failed', 'unsupported')),
  ADD COLUMN IF NOT EXISTS webhook_error TEXT,
  ADD COLUMN IF NOT EXISTS webhook_url TEXT;
