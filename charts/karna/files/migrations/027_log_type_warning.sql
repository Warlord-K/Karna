-- Expand agent_logs.log_type to cover all runtime values currently emitted.
ALTER TABLE agent_logs
  DROP CONSTRAINT IF EXISTS agent_logs_log_type_check;

ALTER TABLE agent_logs
  ADD CONSTRAINT agent_logs_log_type_check
  CHECK (
    log_type IN (
      'info',
      'error',
      'command',
      'output',
      'claude',
      'tool',
      'comment',
      'warning'
    )
  );
