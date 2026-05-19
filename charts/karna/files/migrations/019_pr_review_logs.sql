-- Per-review activity log so the UI can show what the reviewer is doing in
-- real-time (tool calls, assistant text, errors). Mirrors agent_logs in shape.

CREATE TABLE IF NOT EXISTS pr_review_logs (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  review_id   UUID NOT NULL REFERENCES pr_reviews(id) ON DELETE CASCADE,
  phase       TEXT NOT NULL,
  message     TEXT NOT NULL,
  log_type    TEXT,
  metadata    JSONB,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pr_review_logs_review_recent
  ON pr_review_logs (review_id, created_at DESC);
