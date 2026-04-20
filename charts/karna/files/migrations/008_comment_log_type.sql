-- Add 'comment' to allowed log_type values for inline user comments
ALTER TABLE agent_logs DROP CONSTRAINT agent_logs_log_type_check;
ALTER TABLE agent_logs ADD CONSTRAINT agent_logs_log_type_check
  CHECK (log_type IN ('info', 'error', 'command', 'output', 'claude', 'tool', 'comment'));
