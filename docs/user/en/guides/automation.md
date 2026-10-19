---
title: Automation (Scheduled Tasks)
description: 'Compare Scheduled Tasks, loop/schedule skills, and Org automation — and choose the right LibrAgent approach.'
---

# Automation — Scheduled Tasks

> Open **Scheduled Tasks** from the sidebar.  
> Run assistant sessions automatically on a schedule (UI schedule picker or fixed intervals).

---

## Create a New Task

1. Open **Scheduled Tasks** and click **New task** (or **Create your first task**).
2. For instant setup, select one of the provided **Starter Templates** (such as Daily Standup Summary, Code Review Digest, or **Daily durable knowledge distillation**) to pre-fill configuration fields. You can also create the nightly knowledge-distill task in one click from the Chat Hub tip (**Create nightly distill task**).
3. Configure the task fields in the **New Scheduled Task** dialog:

| Field         | Description                                                                                  |
| ------------- | -------------------------------------------------------------------------------------------- |
| **Task name** | Display name shown in the task list.                                                         |
| **Assistant** | The assistant profile assigned to execute the task.                                          |
| **Schedule**  | Recurrence rules and execution times (UI schedule picker or Cron).                           |
| **Workspace** | Optional target folder. Click Browse or drag-and-drop a folder.                              |
| **Message**   | The prompt sent when the task runs. Supports `@playbook:`, `@skill:`, and `@file:` mentions. |

4. Click **Save**. The task card displays the **Next run** timestamp.

Disabled tasks remain in the list without triggering scheduled runs.

---

## Edit or Delete Tasks

Click **Edit Task** or **Delete Task** on any task card.  
If **Scheduled Task Minimum Interval** is configured under **Settings**, short schedules that violate this limit will be rejected to protect system resources.

---

## Comparison of Automation Mechanisms

| Automation Type                                      | Execution Context                                                    |
| ---------------------------------------------------- | -------------------------------------------------------------------- |
| **Scheduled Tasks** (This Screen)                    | App-wide, recurring background runs                                  |
| **`@skill:loop`** ([guide](loop.md))                 | In-session clock wakes, reminders, and timed task-list cycles        |
| **`@skill:call-me-back`** ([guide](call-me-back.md)) | Resumes sessions upon external process or webhook completion signals |
| **`@skill:schedule`**                                | Procedural instructions guiding the agent to manage schedules        |
| **Org / Teamwork**                                   | Explicit hierarchical teams shown in the sidebar **Org** view        |
| **delegate / divide-conquer**                        | Immediate child session delegation during active conversations       |

For details on orchestrating multiple agents, refer to the [Sub-Agents & Orchestration Guide](sub-agents.md).

---

## Best Practices

- Define explicit output formats and deliverables inside the task prompt.
- When tasks require MCP tools, install them via [Extensions](extensions.md) and enable them in the target assistant's tool permissions.
- Inspect execution logs in session **History** if a scheduled task fails.
- Set **Workspace** on a task to run in that folder. Leaving the task Workspace empty does not clear a work folder you already chose in an open chat.

---

## Related Documentation

- [Scheduled Tasks Reference](scheduled-tasks.md) — Scheduling options and syntax
- [Assistants Guide](assistants.md) — Configure dedicated automation personas
- [Playbooks Guide](playbooks.md) — Save repeatable session playbooks
- [Sub-Agents & Orchestration Guide](sub-agents.md) — Multi-agent coordination patterns
- [Troubleshooting Guide](troubleshooting.md) — Diagnose background task failures
