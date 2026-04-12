---
description: Build the project. Auto-detects build system.
phase: implement
---
Detect and run the build:

- **package.json**: `npm run build`
- **Cargo.toml**: `cargo build`
- **go.mod**: `go build ./...`
- **pyproject.toml** with build-system: `python -m build`
- **Makefile**: `make`
- **Dockerfile**: `docker build .`

Verify the build succeeds after changes. Fix any build errors before committing.