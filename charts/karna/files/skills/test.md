---
description: Run the project test suite. Auto-detects framework from project files.
phase: implement
---
Detect the test framework and run tests:

- **package.json** with jest/vitest/mocha: `npx vitest run` or `npx jest --passWithNoTests`
- **pyproject.toml** or **setup.cfg** with pytest: `python -m pytest -x -q`
- **Cargo.toml**: `cargo test`
- **go.mod**: `go test ./...`

Run tests after making changes. If tests fail, fix the issues before committing.
If no test framework is detected, skip this step.