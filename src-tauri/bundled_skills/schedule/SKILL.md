---
name: schedule
description: Create and manage global scheduled tasks in LibrAgent (cron-based). Use when automation should outlive the current session, wake a specific assistant on a cron, or run recurring background work app-wide. For one-shot delays or session-bound recurrence inside the active conversation, use session-schedule instead.
---

# Schedule

Global scheduled tasks wake an assistant later without requiring the current session to stay open. They are not org identity and they are not session-bound follow-ups.

## Routing decision

```text
Should the run stay bound to the current session?
  Yes -> session-schedule (scheduled_task__scheduleCallback)
  No  -> schedule (scheduled_task__createScheduledTask)
```

Recurring vs one-shot is **not** the primary split. Global tasks require cron. Session-bound one-shots use `session-schedule`.

## Workflow

### 1. Confirm global scheduling is appropriate

- Use global tasks for cron-based wake-ups, background recurrence, heartbeat loops, or assistant-level automation.
- Do not require teamwork scaffolding for a standalone global task.
- If the user wants org-visible lineage or org-root resume behavior, stop and use `org`.

### 2. Create the task

**Single global task (default path):**

- Use `scheduled_task__createScheduledTask(name, cronExpression, assistantId, message)`.
- Optional: `scheduleTimezone`, `executionMode`, `workspaceOverride`.
- The tool returns a task ID. Keep it for follow-up calls.

### 3. Operate tasks deliberately

- `scheduled_task__listScheduledTasks()` — discover task IDs and enabled state
- `scheduled_task__getScheduledTask(taskId)` — read message, cron, and pinned session state
- `scheduled_task__updateScheduledTask(taskId, ...)` — retune cadence, message, or workspace
- `scheduled_task__toggleScheduledTask(taskId, enabled=...)` — pause or resume
- `scheduled_task__deleteScheduledTask(taskId)` — remove stale automation

### 4. Respect governance limits

- Backend policy enforces a minimum interval between runs.
- Do not assume unlimited frequency.
- If policy limits are reached, widen the cron cadence or request a limit increase through Settings.

### 5. Keep identity clean

- A scheduled task may wake a coordinator, but it is still not an org.
- Org lineage belongs in org tooling, not scheduled-task metadata.

## Teamwork integration (optional)

When the workspace already uses the scheduled teamwork substrate:

1. If the governing root session has not prepared the teamwork artifact directory yet, stop and use `teamwork` to call `agent__prepareTeamworkWorkspace()` first.
2. Read `.libragent/teamwork.json` if present.
3. Confirm `executionSubstrate.mode` is `"scheduled"` when the user expects recurring teamwork automation driven by scheduled tasks.
4. Ensure teamwork scaffold files (`agents.md`, `MISSION.md`, coordination files) are current before a scheduled coordinator wakes up.
5. Treat the app-local teamwork artifact directory as orchestration SSOT; use `workspaceOverride` only when a run must target a different implementation workspace.

If the user only wants one global cron job with no multi-agent constitution, skip teamwork entirely.

## Guardrails

- Do not use global scheduled tasks for simple in-session delays; use `session-schedule`.
- Use `workspaceOverride` only when the scheduled run must target a specific existing workspace.
- Backend minimum-interval policy still applies. Do not assume unlimited frequency.
