---
description: Create well-structured conventional commits.
phase: implement
---
When committing changes, follow the Conventional Commits format:

```
<type>(<scope>): <short description>

<body - explain WHY, not WHAT>
```

Types:
- **feat**: New feature
- **fix**: Bug fix
- **refactor**: Code restructuring (no behavior change)
- **test**: Adding/updating tests
- **chore**: Build, deps, config changes
- **perf**: Performance improvement
- **ci**: CI/CD changes

Scope: derive from the primary directory or module changed.
Keep the subject line under 72 characters.
Make atomic commits — one logical change per commit.