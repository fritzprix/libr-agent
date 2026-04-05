---
name: team-sprint
description: Run scheduled-task-group teamwork in LibrAgent. Use when collaboration needs recurring, cron-like, heartbeat, or resumable background automation across a shared workspace using grouped scheduled tasks under backend-governed policy.
---

# Team Sprint

Scheduled collaboration is grouped automation. It is not org identity.

Use `task-force-builder` first when the workspace constitution is not ready.

## Workflow

1. Confirm scheduled collaboration is the right substrate.
   - Use it for periodic wake-ups, background recurrence, heartbeat loops, or resumable async automation.
   - If the user wants org-visible lineage or org-root resume behavior, stop and use `team-org`.
2. Reuse the existing workspace constitution.
   - Treat the current workspace scaffold as the SSOT.
   - Scheduled runs should operate against that shared scaffold.
3. Create the task group explicitly.
   - Use `createScheduledTask(...)` with a clear `groupName` for the first loop.
   - Use `createScheduledTask(...)` with the existing `groupId` for additional loops in the same group.
4. Operate the group deliberately.
   - Use `listScheduledTasks()` or `getScheduledTask()` to inspect the current group.
   - Use `updateScheduledTask()` to retune cadence, message, or grouping.
   - Use `toggleScheduledTask()` to pause or resume.
   - Use `deleteScheduledTask()` to remove stale automation.
5. Keep identity clean.
   - A scheduled task may wake the coordinator, but the group is still not an org.
   - Group membership is scheduled-task metadata, not lineage identity.

## Guardrails

- Default to the current workspace scaffold; do not invent a separate workspace just because work is asynchronous.
- Use `workspaceOverride` only when the scheduled run must target a specific existing shared workspace.
- Keep group names stable and readable.
- Backend policy limits still apply. Do not assume unlimited groups or unlimited frequency.
