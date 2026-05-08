-- Add 'cancelled' status for tasks that are no longer needed
ALTER TABLE agent_tasks DROP CONSTRAINT IF EXISTS agent_tasks_status_check;
ALTER TABLE agent_tasks ADD CONSTRAINT agent_tasks_status_check
  CHECK (status IN ('todo', 'planning', 'plan_review', 'in_progress', 'review', 'done', 'failed', 'cancelled'));

-- Treat cancelled like done for timestamp tracking
CREATE OR REPLACE FUNCTION agent_task_status_trigger()
RETURNS TRIGGER AS $$
BEGIN
  IF NEW.status IN ('done', 'cancelled') AND (OLD.status IS NULL OR OLD.status NOT IN ('done', 'cancelled')) THEN
    NEW.completed_at = NOW();
  ELSIF NEW.status NOT IN ('done', 'cancelled') THEN
    NEW.completed_at = NULL;
  END IF;
  IF NEW.status != OLD.status AND NEW.status IN ('planning', 'in_progress') AND OLD.started_at IS NULL THEN
    NEW.started_at = NOW();
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Update parent completion trigger to treat cancelled subtasks as finished
CREATE OR REPLACE FUNCTION check_parent_completion()
RETURNS TRIGGER AS $$
DECLARE
  parent_id UUID;
  total_count INTEGER;
  finished_count INTEGER;
BEGIN
  parent_id := NEW.parent_task_id;
  IF parent_id IS NULL THEN
    RETURN NEW;
  END IF;

  IF NEW.status NOT IN ('done', 'cancelled') THEN
    RETURN NEW;
  END IF;

  SELECT COUNT(*), COUNT(*) FILTER (WHERE status IN ('done', 'cancelled'))
    INTO total_count, finished_count
    FROM agent_tasks
    WHERE parent_task_id = parent_id;

  IF total_count > 0 AND total_count = finished_count THEN
    UPDATE agent_tasks SET status = 'done' WHERE id = parent_id;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;
