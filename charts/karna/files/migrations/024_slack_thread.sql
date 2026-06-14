-- Slack control plane thread mapping.
-- Stores where task updates should be posted in Slack.

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS slack_channel TEXT;

ALTER TABLE agent_tasks
  ADD COLUMN IF NOT EXISTS slack_thread_ts TEXT;
