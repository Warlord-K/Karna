-- Structured findings for PR reviews so the agent can post inline review
-- comments anchored to specific (path, line) pairs instead of one prose blob.
--
-- The reviewer CLI emits a `<!-- findings ... findings -->` JSON block at the
-- end of its run; the agent parses it, validates each anchor against the PR
-- diff, persists the rows here, and POSTs a single review with `comments[]`
-- via `gh api repos/{owner}/{repo}/pulls/{n}/reviews`.
--
-- Findings whose anchors don't appear in the diff (e.g. hallucinated line
-- numbers, files renamed mid-review) are still recorded with posted=false +
-- skip_reason so the UI can show what got dropped.

CREATE TABLE IF NOT EXISTS pr_review_findings (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  review_id     UUID NOT NULL REFERENCES pr_reviews(id) ON DELETE CASCADE,
  path          TEXT NOT NULL,
  -- Ending line of the comment range (inclusive). For single-line comments
  -- this is the only line; for multi-line, start_line < line.
  line          INTEGER NOT NULL,
  start_line    INTEGER,
  -- 'RIGHT' = post-change side (additions/context); 'LEFT' = pre-change side
  -- (deletions). GitHub requires both anchors on the same side.
  side          TEXT NOT NULL DEFAULT 'RIGHT'
    CHECK (side IN ('LEFT', 'RIGHT')),
  body          TEXT NOT NULL,
  posted        BOOLEAN NOT NULL DEFAULT FALSE,
  -- When posted=false, why. NULL when posted=true.
  skip_reason   TEXT,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pr_review_findings_review
  ON pr_review_findings (review_id, created_at);
