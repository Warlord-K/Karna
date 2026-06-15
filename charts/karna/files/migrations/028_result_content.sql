ALTER TABLE agent_tasks
ADD COLUMN IF NOT EXISTS result_content TEXT;
