-- Enable per-repo GitHub issue sync: auto-create tasks from new issues
ALTER TABLE repo_profiles
  ADD COLUMN sync_issues BOOLEAN NOT NULL DEFAULT true;
