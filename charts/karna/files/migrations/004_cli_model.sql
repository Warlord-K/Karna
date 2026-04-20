-- Per-task CLI backend and model selection
ALTER TABLE agent_tasks ADD COLUMN cli TEXT;
ALTER TABLE agent_tasks ADD COLUMN model TEXT;
