---
description: Onboard a new repository — explore its structure, generate a profile summary, and update workspace context.
phase: both
---
When asked to add or onboard a repository:

1. Clone or fetch the repository
2. Explore: README, package manifest, CI config, lint setup, test framework, key entry points, directory structure
3. Generate a structured profile including:
   - Primary language and framework
   - Build/test/lint commands
   - Key directories and entry points
   - CI workflows
4. The profile is stored in the database and used by the planner to route multi-repo tasks efficiently

This skill is triggered automatically when new repos are added to the configuration or via the UI.