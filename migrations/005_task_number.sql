-- Add sequential task number per user (like Linear's ENG-123)
ALTER TABLE agent_tasks ADD COLUMN task_number INTEGER;

-- Backfill existing tasks in creation order per user
WITH numbered AS (
  SELECT id, ROW_NUMBER() OVER (PARTITION BY user_id ORDER BY created_at) AS rn
  FROM agent_tasks
)
UPDATE agent_tasks SET task_number = numbered.rn FROM numbered WHERE agent_tasks.id = numbered.id;

-- Auto-assign on insert
CREATE OR REPLACE FUNCTION assign_task_number()
RETURNS TRIGGER AS $$
BEGIN
  SELECT COALESCE(MAX(task_number), 0) + 1 INTO NEW.task_number
  FROM agent_tasks WHERE user_id = NEW.user_id;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER assign_task_number_trigger
  BEFORE INSERT ON agent_tasks
  FOR EACH ROW
  EXECUTE FUNCTION assign_task_number();

-- Unique per user
CREATE UNIQUE INDEX idx_agent_tasks_user_task_number ON agent_tasks(user_id, task_number);
