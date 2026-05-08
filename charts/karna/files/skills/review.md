---
description: Self-review changes before creating a PR. Check for bugs, security, and quality.
phase: implement
---
Before pushing changes, review your own work:

1. **Run `git diff`** and read every changed line
2. **Check for bugs**: off-by-one errors, null/undefined access, race conditions
3. **Check for security**: SQL injection, XSS, hardcoded secrets, command injection
4. **Check for missing error handling**: unhandled promises, missing try/catch at boundaries
5. **Check for breaking changes**: modified public APIs, changed DB schema without migration
6. **Check test coverage**: are new code paths tested?
7. **Check for leftover debug code**: console.log, print statements, TODO comments

If you find issues, fix them before proceeding.