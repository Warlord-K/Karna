#!/bin/bash
set -euo pipefail

# Migration runner: applies unapplied .sql files from /migrations in order.
# Tracks state in schema_migrations table. Safe to run on every startup.

DB_USER="${POSTGRES_USER:-karna}"
DB_NAME="${POSTGRES_DB:-karna}"
DB_HOST="${POSTGRES_HOST:-postgres}"

psql="psql -h $DB_HOST -U $DB_USER -d $DB_NAME -v ON_ERROR_STOP=1"

$psql -c "
CREATE TABLE IF NOT EXISTS schema_migrations (
  version TEXT PRIMARY KEY,
  applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
"

# Bootstrap: if DB has tables but no migration records, detect what's already applied.
migration_count=$($psql -tAc "SELECT count(*) FROM schema_migrations")
has_tasks_table=$($psql -tAc "SELECT 1 FROM information_schema.tables WHERE table_schema='public' AND table_name='agent_tasks'" || echo "")

if [ "$migration_count" = "0" ] && [ "$has_tasks_table" = "1" ]; then
  echo "bootstrap: existing DB detected, detecting applied migrations"

  # 001: agent_tasks table exists
  $psql -c "INSERT INTO schema_migrations (version) VALUES ('001_initial.sql')"
  echo "bootstrap: marked 001_initial.sql"

  # 002: password column on users
  has_password=$($psql -tAc "SELECT 1 FROM information_schema.columns WHERE table_name='users' AND column_name='password'" || echo "")
  if [ "$has_password" = "1" ]; then
    $psql -c "INSERT INTO schema_migrations (version) VALUES ('002_add_password.sql')"
    echo "bootstrap: marked 002_add_password.sql"
  fi

  # 003: parent_task_id column on agent_tasks
  has_parent=$($psql -tAc "SELECT 1 FROM information_schema.columns WHERE table_name='agent_tasks' AND column_name='parent_task_id'" || echo "")
  if [ "$has_parent" = "1" ]; then
    $psql -c "INSERT INTO schema_migrations (version) VALUES ('003_subtasks.sql')"
    echo "bootstrap: marked 003_subtasks.sql"
  fi

  # 004: cli column on agent_tasks
  has_cli=$($psql -tAc "SELECT 1 FROM information_schema.columns WHERE table_name='agent_tasks' AND column_name='cli'" || echo "")
  if [ "$has_cli" = "1" ]; then
    $psql -c "INSERT INTO schema_migrations (version) VALUES ('004_cli_model.sql')"
    echo "bootstrap: marked 004_cli_model.sql"
  fi
fi

# Apply each unapplied migration in order
for f in /migrations/[0-9]*.sql; do
  version=$(basename "$f")
  already_applied=$($psql -tAc "SELECT 1 FROM schema_migrations WHERE version = '$version'")
  if [ "$already_applied" = "1" ]; then
    echo "skip: $version (already applied)"
  else
    echo "applying: $version"
    $psql -f "$f"
    $psql -c "INSERT INTO schema_migrations (version) VALUES ('$version')"
    echo "applied: $version"
  fi
done

echo "migrations complete"
