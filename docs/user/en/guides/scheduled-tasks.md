---
title: Scheduled Tasks
---

# Scheduled Tasks

Scheduled Tasks allow agents to automatically run recurring prompts and workflows in the background according to a defined schedule (Cron expressions).

---

## 🌟 Key Capabilities

- **Cron-Based Scheduling**: Supports flexible intervals (e.g., daily at 9am, weekly on Mondays).
- **Execution Modes**:
  - `YOLO`: Auto-approves standard tool calls for unattended background execution.
  - `Unsafe`: Completely unattended Full Auto mode allowing shell commands and file operations without confirmation.
  - `Normal`: Prompts for user approval before sensitive operations.
- **Dedicated Assistant Assignment**: Bind tasks to specialized assistants with tailored instructions and tools.
- **One-Click Starter Templates**: Pre-configured tasks ready to deploy.

---

## 🚀 Starter Templates

| Template                                | Schedule               | Mode                 | Summary                                                                                          |
| --------------------------------------- | ---------------------- | -------------------- | ------------------------------------------------------------------------------------------------ |
| **PC Health & Security Daily Audit**    | Midnight (`0 0 * * *`) | `Unsafe` (Full Auto) | Checks disk space, long-running processes, uncommitted Git changes, and package vulnerabilities. |
| **Website Changes & Headline Briefing** | 9:00 AM (`0 9 * * *`)  | `YOLO`               | Uses browser automation to visit tech news sites and generate a 3-bullet summary.                |

---

## ⚙️ Managing Scheduled Tasks

1. Navigate to **Scheduled Tasks** in the left sidebar (clock icon).
2. Click **Add Task**.
3. Configure the name, Cron expression, prompt message, and execution mode (`YOLO`, `Unsafe`, or `Normal`).
4. Toggle tasks on/off or click **Run Now** to test execution immediately.
