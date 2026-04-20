-- Add 'tool' to allowed log_type values
ALTER TABLE agent_logs DROP CONSTRAINT agent_logs_log_type_check;
ALTER TABLE agent_logs ADD CONSTRAINT agent_logs_log_type_check
  CHECK (log_type IN ('info', 'error', 'command', 'output', 'claude', 'tool'));
