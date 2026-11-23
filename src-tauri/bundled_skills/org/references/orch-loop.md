# Orch Loop (org reference)

Run an existing org on a clock: the org root acts as PM, and each tick hands board work to org members. Use when the user asks to run an org continuously (for example "run the org every 10 minutes with 3 members at a time").

The org scaffold is the operating contract: `agents.md`, `MISSION.md`, `ROLES.md`, `coordination/`, role skills, and `@teamwork/.libragent/teamwork.json`. Roles, the board, how status is recorded or synced, and the definition of done all come from it. An explicit user instruction replaces the matching part of the scaffold.

## Parameters

Get two values from the user request: `interval` (the root tick period) and `slots` (the number of members working at once). This is not a slash command; the user asks in plain words. Record `slots` in `coordination/DECISIONS.md` on first run; ask once if it is missing. `slots + 1` fits the app's active-agent limit (default 4).

## Start

1. Read `@teamwork/.libragent/teamwork.json`. The current session must be `executionSubstrate.orgLineage.rootSessionId`; otherwise create or resume the org with the main `org` workflow first.
2. Read `agents.md`, `MISSION.md`, `ROLES.md`, and `coordination/`.
3. Schedule the tick on the root session (see the `loop` skill): one `scheduled_task__scheduleCallback(message, cronExpression)` on the root. Reuse the root callback shown by `scheduled_task__listScheduledTasks()`. To stop the loop, delete or pause that callback (`scheduled_task__deleteScheduledTask` / `scheduled_task__toggleScheduledTask`).

## Tick

1. Read the board the scaffold names (default `coordination/KANBAN.md`). With nothing actionable, end the tick.
2. For each actionable item, up to `slots`: take the role `ROLES.md` assigns and the member the scaffold names for it (`ROLES.md`, `coordination/`); check that member's status and session id in `agent__getOrg()` (it reports `name [status] depth=N session=ID`); if the member is Idle, send `agent__messageToSession(sessionId, complexity, message, reset=true, waitForResponse=false)`; otherwise `agent__spawnSession(configId, complexity, task)`. Complexity comes from the role.
3. The message asks the member to do one unit of its role per `agents.md` and its role skill, record status as the scaffold says, and finish.
4. A member waiting on the user: record it as blocked on the board, tell the user, and free the slot.
5. Close items judged complete under `MISSION.md`'s definition of done, recording it as the scaffold says.
