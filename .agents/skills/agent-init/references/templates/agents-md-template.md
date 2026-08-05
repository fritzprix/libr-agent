# 🚀 Project Guidelines & Entrypoint (`agents.md`)

> **Note for AI Agents**: This file is a **lightweight routing index and quick-reference guide**.
> Do not bloat this file with detailed implementations or long descriptions.
> For specific tasks, selectively read the relevant detailed guide files linked below.

---

## 📌 Project Overview

- **Name**: `{project_name}`
- **Purpose**: `{project_purpose}`
- **Tech Stack**: `{tech_stack_summary}`

---

## 🎭 Persona & User Directives (Strict)

> **High Priority**: Always adhere to these directives and persona across all tasks.

- **Persona / Tone**: `{persona_summary}`
- **User Directives**: `{user_directives_summary}`
- **Strict Precautions**: `{strict_precautions_summary}`

_For full persona details, user preferences, and prohibited patterns, see [`docs/guidelines/persona-and-rules.md`](docs/guidelines/persona-and-rules.md)._

---

## 🛠️ Essential Command Cheat Sheet

| Category          | Command                | Note                     |
| ----------------- | ---------------------- | ------------------------ |
| **Dev Server**    | `{dev_command}`        | Start local development  |
| **Build**         | `{build_command}`      | Compile / bundle         |
| **Test**          | `{test_command}`       | Run tests                |
| **Lint / Format** | `{lint_command}`       | Code quality check       |
| **Validation**    | `{validation_command}` | Full validation pipeline |

---

## 📚 Modular Guides (Read On-Demand)

When working on specific tasks, read only the relevant guide file to preserve context:

| Guide File                                                                               | Focus Area                                                                | When to Read                                                                    |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| [`docs/guidelines/architecture-and-files.md`](docs/guidelines/architecture-and-files.md) | System architecture, module map, key file paths                           | Before starting refactoring, adding new modules, or locating files              |
| [`docs/guidelines/coding-standards.md`](docs/guidelines/coding-standards.md)             | Coding style, type safety, error handling, logging, testing               | When writing code, creating components, or implementing backend logic           |
| [`docs/guidelines/persona-and-rules.md`](docs/guidelines/persona-and-rules.md)           | User directives, desired persona, strict precautions, prohibited patterns | When checking tone/vibe, verifying compliance, or handling sensitive operations |
| [`docs/guidelines/workflows.md`](docs/guidelines/workflows.md)                           | Dev workflows, CI/CD, PR guidelines, release process                      | When running pipelines, preparing PRs, or releasing                             |

---

## 🔄 Self-Healing & Immediate Update Protocol

1. **Detect Drift**: If you discover new files, changed commands, refactored modules, or updated user instructions during your work:
2. **Update Immediately**: Immediately edit the relevant modular guide in `docs/guidelines/`.
3. **Sync Entrypoint**: Update this `agents.md` file if the high-level cheat sheet or module index changed.
