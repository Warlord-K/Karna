-- Auto-review human-opened PRs.
--
-- When a PR is opened (or force-pushed) on a repo whose profile has
-- `review_prs = TRUE`, the agent runs a read-only review pass and posts
-- a single review comment via `gh pr review`. Uses the user's existing
-- Claude/Codex subscription — no extra API spend.
--
-- Dedupe: UNIQUE (repo, head_sha) means re-running on the same commit
-- is a no-op, but force-pushes (new head_sha) get a fresh review.

CREATE TABLE IF NOT EXISTS pr_reviews (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  repo              TEXT NOT NULL,
  pr_number         INTEGER NOT NULL,
  pr_url            TEXT,
  head_sha          TEXT NOT NULL,
  author            TEXT,
  -- Which agent profile reviewed it (NULL if the profile was deleted later).
  reviewer_agent_id UUID REFERENCES agent_profiles(id) ON DELETE SET NULL,
  status            TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'running', 'completed', 'failed', 'skipped')),
  comments_posted   INTEGER NOT NULL DEFAULT 0,
  cost_usd          DOUBLE PRECISION NOT NULL DEFAULT 0,
  error_message     TEXT,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  completed_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_pr_reviews_dedupe
  ON pr_reviews (repo, head_sha);

CREATE INDEX IF NOT EXISTS idx_pr_reviews_repo_recent
  ON pr_reviews (repo, created_at DESC);

-- Per-repo configuration: opt-in flag + optional override of which agent reviews.
ALTER TABLE repo_profiles
  ADD COLUMN IF NOT EXISTS review_prs BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN IF NOT EXISTS review_agent_id UUID
    REFERENCES agent_profiles(id) ON DELETE SET NULL;
