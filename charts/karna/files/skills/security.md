---
description: Run security scans and dependency audits.
phase: implement
---
Run security checks appropriate to the project:

- **npm**: `npm audit --production`
- **Python**: `pip-audit` or `safety check`
- **Rust**: `cargo audit`
- **Go**: `govulncheck ./...`

Also check for:
- Hardcoded secrets (API keys, passwords, tokens in source code)
- SQL injection vulnerabilities (raw string concatenation in queries)
- XSS in rendered user input
- Command injection via unsanitized shell calls
- Insecure dependencies with known CVEs

If critical vulnerabilities are found, flag them in the PR description.