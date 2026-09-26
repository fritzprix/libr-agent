---
name: call-me-back
description: >-
  Resume this agent session on a completion signal instead of clock polling.
  Use when running long builds/tests/training in the background, waiting for
  process exit, kanban/ticket status (e.g. uvx ak5), webhooks, file watchers,
  or git hooks — "call me back when done", "끝나면 깨워줘", "백그라운드로
  돌리고 끝나면 알려줘", "티켓 Done 되면 콜백", "run in background with hook".
  NEVER for pure time reminders or cron ("5분 뒤", "매일 9시") — use loop
  (session) or schedule (global). "me" means this agent session, not a phone call.
---

# Call Me Back (event-driven resume)

Wait for an **external or process completion signal**, then continue this
session. Do not fake completion with `scheduleCallback` / `loop` delays.

**"me"** = this agent session (inject/resume here), not a user phone/SMS alert.

## vs `loop` / `schedule` (hard split)

| Signal | Skill |
| --- | --- |
| Process exit, kanban/ticket, webhook, watcher, hook | **`call-me-back`** (this skill) |
| Relative delay / wall-clock / session cron | **`loop`** |
| Permanent app-wide cron | **`schedule`** |

If the user says "끝나면 알려줘" and names a **command, job, ticket, or board**,
use this skill. If they only name a **clock time**, use `loop`.

## Typical jobs

- Heavy local jobs: builds, tests, training, long scripts
- Background process already started via `workspace__spawnProcess`
- External orchestration: AK5 / other kanban ticket moves to Done / blocked / mention
- Watchers and hooks that fire once on a real event

## Out of scope

- "Remind me in 10 minutes" / "내일 9시에" → `loop`
- Always-on global cron with no session binding → `schedule`
- Immediate child-session work with no wait → `delegate`

## Workflow

### A. Local long-running command

1. Start (or reuse) a background process:
   `workspace__spawnProcess` (or a sync tool that handed off a `processId`).
2. Block on completion — do **not** poll with `loop`:
   `workspace__waitForProcess(processId=..., timeout=...)`
   (`waitForProcess` wakes on process notify; it is the built-in call-me-back for shells.)
3. On finish, read output if needed:
   `workspace__readProcessOutput(processId=...)`
4. Continue the user task with the result (pass/fail, logs, next steps).

Prefer one `waitForProcess` over repeated short waits or clock callbacks.

### B. External systems (kanban / AK5 / webhooks)

1. Identify the concrete signal (ticket id, board, status, mention, webhook).
2. Use that system's event or wait API when available (MCP tools, `uvx ak5 …`,
   board `--watch` under `spawnProcess` + `waitForProcess`, etc.).
3. On the signal, update status/comment if the workflow expects it, then resume
   the original LibrAgent task in **this** session.
4. **Never** approximate "when ticket Done" with `loop` `delaySeconds`.

If the external system only offers coarse polling, prefer its native watch/CLI
over LibrAgent clock loops, and keep poll intervals honest in the reply.

### C. Ambiguous "끝나면 알려줘"

- Named process/command/ticket/board → this skill
- Only a duration or clock time → `loop`
- Still unclear → ask once: time-based reminder, or wait for a specific completion?

## Guardrails

- Do not use `scheduled_task__scheduleCallback` as a stand-in for job completion.
- Do not busy-poll via many tiny `loop` delays or LLM turn spam.
- Do not invent `processId` values; only use ids returned by workspace tools.
- Do not treat this skill as user push-notification / telephony.
- Keep session context: completion should resume **this** chat unless the user
  explicitly asked for a global scheduled worker (`schedule`).

## Related skills

- **`loop`** — clock-based session reminders and recurring checks
- **`schedule`** — global cron automation
- **`delegate`** — spawn work in a child session now
- **`teamwork` / `org`** — multi-agent constitution; not required for a single wait
