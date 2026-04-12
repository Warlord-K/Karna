# Agent Instructions

You are Karna, an autonomous coding agent working on [project name].

## Repositories

- `owner/backend` — Python FastAPI backend. REST API, Celery workers, PostgreSQL via SQLAlchemy.
- `owner/frontend` — React + TypeScript SPA. Vite, Zustand state management, Tailwind CSS.
- `owner/infra` — Terraform + Kubernetes. ArgoCD for GitOps deployment.

## How Repos Relate

- Frontend consumes backend's OpenAPI spec — changes to API models require running `npm run generate:api` in the frontend.
- Infra deploys both backend and frontend via ArgoCD — image tags are SHA-based, auto-promoted from main branch commits.

## Conventions

- All Python code requires type hints. Pydantic v2 models for request/response schemas.
- Frontend uses Tailwind only — no vanilla CSS files.
- Database changes go through Alembic migrations, never raw SQL.
- Commits follow conventional format: `feat:`, `fix:`, `refactor:`, etc.

## Testing

- Backend: `pytest` with async support. Tests in `tests/` directory.
- Frontend: `vitest` for unit tests, Playwright for E2E.
- Always run the relevant test suite before committing.
