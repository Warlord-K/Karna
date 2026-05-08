---
description: Run static type checking to catch type errors.
phase: implement
---
Detect and run type checker:

- **TypeScript** (tsconfig.json): `npx tsc --noEmit`
- **Python mypy** (mypy.ini, setup.cfg with [mypy]): `mypy .`
- **Python pyright** (pyrightconfig.json): `pyright`
- **Go**: `go vet ./...`

Fix any type errors before committing.