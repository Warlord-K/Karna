-- Add password column for email/password auth (idempotent)
ALTER TABLE users ADD COLUMN IF NOT EXISTS password TEXT;
