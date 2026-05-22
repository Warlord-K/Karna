-- Severity tier for each PR review finding so reviewers can scan a long PR
-- and see what actually needs attention vs. nice-to-fix nits.
--
-- The reviewer CLI emits `severity` ("high", "medium", "low") alongside each
-- finding. The harness prepends a labeled marker to the comment body so the
-- severity is visible on GitHub itself, and the UI renders a colored badge.
--
-- Existing rows backfill to 'medium' so legacy reviews continue to render.

ALTER TABLE pr_review_findings
  ADD COLUMN IF NOT EXISTS severity TEXT NOT NULL DEFAULT 'medium'
    CHECK (severity IN ('high', 'medium', 'low'));
