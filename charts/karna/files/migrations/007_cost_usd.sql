-- Track accumulated cost (USD) per task across all CLI invocations
ALTER TABLE agent_tasks ADD COLUMN cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0;
