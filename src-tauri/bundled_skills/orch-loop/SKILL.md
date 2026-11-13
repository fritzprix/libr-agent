---
name: orch-loop
description: "Run an existing LibrAgent org on a clock: the org root acts as PM and each tick hands work to org members (`/orch-loop 10m slots: 3`). Use when the user asks to run an org continuously or triggers `/orch-loop`."
---

# Orch Loop

The org scaffold is the operating contract: `agents.md`, `MISSION.md`, `ROLES.md`, `coordination/`, role skills, and `@teamwork/.libragent/teamwork.json`. Roles, the board, how status is recorded or synced, and the definition of done all come from it. An explicit user instruction replaces the matching part of the scaffold.

## Invocation

`/orch-loop <interval> slots: <N>`: `interval` is the root tick period; `slots` is the number of members working at once. Record `slots` in `coordination/DECISIONS.md` on first run; ask once if it is missing. `slots + 1` fits the app's active-agent limit (default 4).

## Start

1. Read `@teamwork/.libragent/teamwork.json`. The current session is `executionSubstrate.orgLineage.rootSessionId`; otherwise use `org`.
2. Read `agents.md`, `MISSION.md`, `ROLES.md`, and `coordination/`.
3. Schedule the tick with `loop`: one `scheduled_task__scheduleCallback(message, cronExpression)` on the root. Reuse the root callback shown by `scheduled_task__listScheduledTasks()`.

## Tick

1. Read the board the scaffold names (default `coordination/KANBAN.md`). With nothing actionable, end the tick.
2. For each actionable item, up to `slots`: take the role `ROLES.md` assigns and the member the scaffold names for it (`ROLES.md`, `coordination/`); check that member's status and session id in `agent__getOrg()` (it reports `name [status] depth=N session=ID`); if the member is Idle, send `agent__messageToSession(sessionId, complexity, message, reset=true, waitForResponse=false)`; otherwise `agent__spawnSession(configId, complexity, task)`. Complexity comes from the role.
3. The message asks the member to do one unit of its role per `agents.md` and its role skill, record status as the scaffold says, and finish.
4. A member waiting on the user: record it as blocked on the board, tell the user, and free the slot.
5. Close items judged complete under `MISSION.md`'s definition of done, recording it as the scaffold says.
