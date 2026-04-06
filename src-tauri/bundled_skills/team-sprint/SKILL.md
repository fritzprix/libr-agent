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
2. Read `.libragent/teamwork.json` before acting.
   - Confirm `executionSubstrate.mode` is `"scheduled"` and `scheduledTaskGroups.intended` is `true`.
   - If no manifest exists yet, run `task-force-builder` first to scaffold the workspace constitution.
3. Check whether the workspace scaffold exists.
   - If `agents.md`, `MISSION.md`, or `coordination/KANBAN.md` are missing, repair the scaffold before creating scheduled tasks.
   - A master agent woken by a scheduled task must verify the scaffold is present and current before issuing directives.
4. Reuse the existing workspace constitution.
   - Treat the current workspace scaffold as the SSOT.
   - Scheduled runs should operate against that shared scaffold.
5. Create the task group explicitly.
   - Use `createScheduledTask(message, cronExpression, groupName, agentId)` for the first loop in a new group.
   - The `groupName` must be stable, readable, and unique within the team. Record it in `coordination/DECISIONS.md`.
   - Use `createScheduledTask(message, cronExpression, groupName, agentId, groupId)` to add subsequent tasks to the same group.
   - Record the returned `groupId` in `.libragent/teamwork.json` under `scheduledTaskGroups.groupId` so it is available across sessions.
6. Operate the group deliberately.
   - Use `listScheduledTasks()` or `getScheduledTask(taskId)` to inspect the current group.
   - Use `updateScheduledTask(taskId, ...)` to retune cadence, message, or grouping.
   - Use `toggleScheduledTask(taskId)` to pause or resume.
   - Use `deleteScheduledTask(taskId)` to remove stale automation.
7. Respect governance limits.
   - Backend policy enforces a maximum number of scheduled task groups and a minimum interval between runs.
   - Do not assume unlimited groups or unlimited frequency. Check the current policy before creating new groups.
   - If policy limits are reached, consolidate existing groups or request a limit increase through Settings.
8. Keep identity clean.
   - A scheduled task may wake the coordinator, but the group is still not an org.
   - Group membership is scheduled-task metadata, not lineage identity.

## Guardrails

- Default to the current workspace scaffold; do not invent a separate workspace just because work is asynchronous.
- Use `workspaceOverride` only when the scheduled run must target a specific existing shared workspace.
- Keep group names stable and readable. Changing a `groupName` mid-run makes group tracking unreliable.
- Always record `groupId` in `.libragent/teamwork.json` after the first task in a group is created.
- Backend policy limits still apply. Do not assume unlimited groups or unlimited frequency.
- Do not use scheduled task groups as a substitute for org identity.
