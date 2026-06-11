---
name: schedule
description: Create and manage global scheduled tasks in LibrAgent (single or grouped, cron-based). Use when automation should outlive the current session, wake a specific assistant on a cron, or run recurring background work app-wide. For one-shot delays or session-bound recurrence inside the active conversation, use session-schedule instead.
---

# Schedule

Global scheduled tasks wake an assistant later without requiring the current session to stay open. They are not org identity and they are not session-bound follow-ups.

## When to use this skill

Use `schedule` when:

- the work should continue after the current session ends or is closed
- the user wants app-wide recurring automation (daily digest, weekly report, heartbeat)
- a specific assistant must run on a cron, with or without a task group
- teamwork chose the `scheduled` execution substrate and needs grouped automation loops

Stop and use `session-schedule` when:

- the user wants a delay or reminder inside **this conversation** ("check back in 5 minutes", "continue tomorrow morning in this thread")
- the injected message must resume the **current** session context

Stop and use `org` when the real need is org-visible lineage, not cron automation.

Stop and use `teamwork` first only when you are setting up a multi-agent workspace constitution and have not scaffolded it yet.

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
- Single tasks are normal. Groups are optional.
- Do not require teamwork scaffolding for a standalone global task.
- If the user wants org-visible lineage or org-root resume behavior, stop and use `org`.

### 2. Create the task

**Single global task (default path):**

- Use `scheduled_task__createScheduledTask(name, cronExpression, assistantId, message)`.
- Optional: `scheduleTimezone`, `yoloMode`, `workspaceOverride`.
- The tool returns a task ID. Keep it for follow-up calls.

**Grouped automation (teamwork / scheduled substrate only):**

- Use when multiple related loops should be managed together under backend group policy.
- First task in a group: `scheduled_task__createScheduledTask(..., groupName="...")`.
- Additional tasks in the same group: pass the returned `groupId`.
- If `.libragent/teamwork.json` exists with `executionSubstrate.mode = "scheduled"`, record `groupId` there for cross-session visibility.
- If no teamwork manifest exists, groups still work; manifest recording is optional convenience, not a prerequisite.

### 3. Operate tasks deliberately

- `scheduled_task__listScheduledTasks()` — discover task IDs and enabled state
- `scheduled_task__getScheduledTask(taskId)` — read message, cron, group, and pinned session state
- `scheduled_task__updateScheduledTask(taskId, ...)` — retune cadence, message, grouping, or workspace
- `scheduled_task__toggleScheduledTask(taskId, enabled=...)` — pause or resume
- `scheduled_task__deleteScheduledTask(taskId)` — remove stale automation

### 4. Respect governance limits

- Backend policy enforces a maximum number of scheduled task groups and a minimum interval between runs.
- Do not assume unlimited groups or unlimited frequency.
- If policy limits are reached, consolidate existing groups or request a limit increase through Settings.

### 5. Keep identity clean

- A scheduled task may wake a coordinator, but a task group is still not an org.
- Group membership is scheduled-task metadata, not lineage identity.

## Teamwork integration (optional)

When the workspace already uses the scheduled teamwork substrate:

1. If the governing root session has not prepared the teamwork artifact directory yet, stop and use `teamwork` to call `agent__prepareTeamworkWorkspace()` first.
2. Read `.libragent/teamwork.json` if present.
3. Confirm `executionSubstrate.mode` is `"scheduled"` when the user expects grouped collaboration automation.
4. Ensure teamwork scaffold files (`agents.md`, `MISSION.md`, coordination files) are current before a scheduled coordinator wakes up.
5. Treat the app-local teamwork artifact directory as orchestration SSOT; use `workspaceOverride` only when a run must target a different implementation workspace.

If the user only wants one global cron job with no multi-agent constitution, skip teamwork entirely.

## Guardrails

- Do not use global scheduled tasks for simple in-session delays; use `session-schedule`.
- Do not use scheduled task groups as a substitute for org identity.
- Keep `groupName` stable when using groups. Renaming mid-run makes tracking unreliable.
- Use `workspaceOverride` only when the scheduled run must target a specific existing workspace.
- Backend policy limits still apply. Do not assume unlimited groups or unlimited frequency.
