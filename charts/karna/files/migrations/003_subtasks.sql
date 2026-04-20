-- Subtask support: parent_task_id + nullable repo

-- Make repo optional (parent tasks may not target a specific repo)
ALTER TABLE agent_tasks ALTER COLUMN repo DROP NOT NULL;

-- Add parent_task_id for subtask hierarchy
ALTER TABLE agent_tasks ADD COLUMN parent_task_id UUID REFERENCES agent_tasks(id) ON DELETE CASCADE;

-- Index for fast subtask lookups
CREATE INDEX IF NOT EXISTS idx_agent_tasks_parent_id ON agent_tasks(parent_task_id);

-- Auto-complete parent when all subtasks are done
CREATE OR REPLACE FUNCTION check_parent_completion()
RETURNS TRIGGER AS $$
DECLARE
  parent_id UUID;
  total_count INTEGER;
  done_count INTEGER;
BEGIN
  parent_id := NEW.parent_task_id;
  IF parent_id IS NULL THEN
    RETURN NEW;
  END IF;

  -- Only trigger when a subtask moves to done
  IF NEW.status != 'done' THEN
    RETURN NEW;
  END IF;

  SELECT COUNT(*), COUNT(*) FILTER (WHERE status = 'done')
    INTO total_count, done_count
    FROM agent_tasks
    WHERE parent_task_id = parent_id;

  IF total_count > 0 AND total_count = done_count THEN
    UPDATE agent_tasks SET status = 'done' WHERE id = parent_id;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER check_parent_completion_trigger
  AFTER UPDATE ON agent_tasks
  FOR EACH ROW
  WHEN (NEW.parent_task_id IS NOT NULL)
  EXECUTE FUNCTION check_parent_completion();
