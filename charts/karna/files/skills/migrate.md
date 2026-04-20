---
description: Create and run database migrations. Auto-detects ORM/migration tool.
phase: both
---
When the plan involves database schema changes, detect the migration tool:

- **Supabase** (supabase/ dir): Create file in `supabase/migrations/YYYYMMDD_name.sql`
- **Prisma** (prisma/schema.prisma): `npx prisma migrate dev --name <name>`
- **Drizzle** (drizzle.config.*): `npx drizzle-kit generate` then `npx drizzle-kit push`
- **Alembic** (alembic.ini): `alembic revision --autogenerate -m "<name>"`
- **Django** (manage.py): `python manage.py makemigrations` then `python manage.py migrate`
- **SQLAlchemy** (without Alembic): Create migration SQL manually
- **Knex** (knexfile.*): `npx knex migrate:make <name>`

Rules:
- Always create a new migration file, never modify existing ones
- Include both forward changes (what SQL does) — down/rollback only if the tool supports it
- Test the migration can be applied cleanly