---
description: Run linter and auto-fix issues. Auto-detects from project config.
phase: implement
---
Detect the linter and run with auto-fix:

- **ESLint** (.eslintrc*, eslint.config.*): `npx eslint --fix .`
- **Ruff** (ruff.toml, pyproject.toml with [tool.ruff]): `ruff check --fix .`
- **Clippy**: `cargo clippy --fix --allow-dirty`
- **golangci-lint** (.golangci.yml): `golangci-lint run --fix`

Run after making code changes but before committing.