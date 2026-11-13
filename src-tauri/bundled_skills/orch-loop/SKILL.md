---
name: orch-loop
description: "Run recurring PM orchestration on top of an existing LibrAgent org (`/orch-loop 10m slots: 3`). Use when the user asks to run PM orchestration over an org, delegate recurring repo-loop ticks to org members, or triggers `/orch-loop`."
---

# Orch Loop

Thin PM orchestration adapter over an existing `org`. It delegates work to org members and defers all role behavior, scoring, and PR/issue rules to the loaded role skills.

## Invocation

`/orch-loop <interval> slots: <N>` (for example `/orch-loop 10m slots: 3`).

- `interval` controls the PM tick on the **org root only**.
- `slots` is the total concurrent member cap. Persist it in `.libragent/teamwork.json` (or `coordination/` progress notes) after the first run; ask once if missing on first invocation.
- Validate `slots` against the app-wide concurrency gate (default `4` active / `8` waiting) and lineage fanout limits, leaving room for the org root.

## Prerequisites & Exclusivity

1. **Verify org root:** Confirm `.libragent/teamwork.json` has `executionSubstrate.mode == "org"` and the current session is the org root (`rootSessionId`). If not, point the user to `org` / `teamwork` and stop.
2. **Single schedule on root:** Only the org root holds `scheduled_task__scheduleCallback`. Check `scheduled_task__listScheduledTasks()` first so no duplicate PM cron or standalone repo-loop scheduled tasks (`worker-loop` / `gatekeeper-loop` / `producer-loop`) collide on the same repo. Member sessions never run their own crons.

## Tick Workflow

1. **Cheap NOOP check:** Inspect GitHub board state (open PRs, `approved` issues, `auto:producer` / `validated` queues). If nothing is actionable, exit cleanly as NOOP without waking members.
2. **Map members to slots:** Inspect `agent__listAgents(type="sessions")` for org children within `slots`:
   - Reuse an `Idle` org member with `agent__messageToSession(sessionId, message, reset=true)` before spawning a new one.
   - Spawn via `agent__spawnSession(configId, complexity, task)` only when no suitable `Idle` member exists and `slots` allows.
   - Prefer strong models (`high` / `normal` complexity) for PM and `gatekeeper-loop` roles; weaker routing only where acceptable (e.g., `producer-loop`).
3. **Assign one-tick role work:** Instruct each member to run **one tick** of its existing role skill (`worker-loop`, `gatekeeper-loop`, or `producer-loop`), setting a distinct `WORKER_LOOP_SESSION_ID` per member (since the app PID is shared across sessions).
   - Note: `gatekeeper-loop` is the repo PR/issue reviewer loop — do **not** confuse it with the bundled `gatekeeper` creator–reviewer skill.
   - Defer gatekeeper scoring/thresholds to whichever `gatekeeper-loop` skill is loaded; do not encode policy here.
4. **Handle blocked approvals:** If a member blocks on a pending tool/permission approval (normal mode waits indefinitely; yolo still blocks on hard approvals), escalate to the user as `needs-human` and reclaim the slot instead of holding it.

## Guardrails

- **No policy in teamwork artifacts:** `coordination/` and `.libragent/teamwork.json` hold only skill references, `slots`, and progress notes. Authoritative state stays on GitHub (labels, comments, markers) so it survives app-data loss.
- **Never approve issues or merge PRs:** Issue approval and PR merging remain human-only actions.
- **Workspace `ROLES.md`:** Point roles only at existing skills (`worker-loop`, `gatekeeper-loop`, `producer-loop`); never redefine role rules in `orch-loop`.
