-- Optional discriminator for task entrypoint/surface.
-- NULL (default) keeps existing board behavior.
ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS source TEXT;
