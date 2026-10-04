---
title: Scheduled Tasks Reference
---

# Scheduled Tasks Reference

Scheduled Tasks allow agents to automatically run recurring prompts and workflows in the background according to a defined schedule (Cron expressions).

---

## 🌟 Key Capabilities

- **Cron-Based Scheduling**: Supports precise intervals (e.g., daily at 09:00, weekly on Mondays).
- **Execution Modes**:
  - `YOLO`: Auto-approves standard tool calls for unattended background execution.
  - `Unsafe`: Completely unattended Full Auto mode allowing shell commands and file operations without confirmation dialogs.
  - `Normal`: Pauses and prompts for user approval before sensitive operations.
- **Dedicated Assistant Assignment**: Binds tasks to specialized assistants with tailored instructions and tool permissions.
- **Starter Templates**: Provides 1-click deployment for common maintenance tasks.

---

## 🚀 Starter Templates

Deploy pre-configured tasks directly from the Scheduled Tasks view:

| Template Name | Schedule | Execution Mode | Summary |
| --- | --- | --- | --- |
| **PC Health & Security Daily Audit** | Daily at Midnight (`0 0 * * *`) | `Unsafe` (Full Auto) | Inspects available disk space, long-running processes, uncommitted Git changes, and package vulnerabilities. |
| **Website Changes & Headline Briefing** | Daily at 09:00 (`0 9 * * *`) | `YOLO` | Uses browser automation to visit tech sites and generate a 3-bullet summary briefing. |

---

## ⚙️ Create and Manage Tasks

1. Navigate to **Scheduled Tasks** in the sidebar (clock icon).
2. Click **New task** or **Add Task**.
3. Configure the task properties:
   - **Task Name**: Descriptive title (e.g., `Daily Backup Check`).
   - **Cron Expression**: Recurrence schedule (e.g., `0 9 * * 1-5` for weekdays at 09:00).
   - **Prompt Message**: Clear instructions sent to the agent.
   - **Assistant**: The assistant persona assigned to execute the prompt.
   - **Execution Mode**: Choose `YOLO`, `Unsafe`, or `Normal` based on required automation level.
4. **Save and Activate**:
   - Use the toggle switch on the card to enable or disable the task at any time.
   - Click **Run Now** to test execution immediately without waiting for the scheduled time.

---

## 💡 Important Considerations

- **Local Desktop Execution**: Because LibrAgent runs locally on your computer, your machine must be powered on and LibrAgent must remain active in the background for scheduled tasks to execute.
- **Unattended Execution**: For completely unattended runs that modify files or execute shell commands, select `YOLO` or `Unsafe` mode to prevent tasks from pausing on approval dialogs.

---

## Related Documentation

- [Automation Overview Guide](automation.md) — Automation architecture and workflow comparison
- [Assistants Guide](assistants.md) — Create dedicated automation personas
- [Playbooks Guide](playbooks.md) — Reusable execution templates
- [Troubleshooting Guide](troubleshooting.md) — Diagnose background task errors
