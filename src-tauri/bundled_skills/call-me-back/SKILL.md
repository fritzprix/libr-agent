---
name: call-me-back
description: >-
  Resume this agent session on a signal instead of clock polling. Use when you
  need to (1) wait for a process this session started to finish — "끝나면 깨워줘",
  "완료되면 알려줘", "run in background and call me back"; (2) react to events
  that arrive from outside — "X가 오면 Y 해줘", "새 이벤트가 생기면 처리해줘",
  "when a new one comes in, handle it" — by wiring a hook/webhook/watcher that
  POSTs to $LIBRAGENT_WAKE_URL. Arriving events → this skill; checking on a
  time interval → loop. NEVER for pure time reminders or cron ("5분 뒤",
  "매일 9시") — use loop (session) or schedule (global). "me" means this agent
  session, not a phone call.
---

# Call Me Back (event-driven resume)

Wait for a **process completion or an incoming external event**, then continue
this session. Do not fake it with `scheduleCallback` / `loop` delays.

**"me"** = this agent session (inject/resume here), not a user phone/push alert.

## First: what are you waiting for?

| Waiting for | Path |
| --- | --- |
| A process **this session** started (`runShell` / `spawnProcess` gave you a `processId`), result needed this turn | **A** — `workspace__waitForProcess` |
| A process **this session** started, runs many minutes, nothing to do meanwhile | **A2** — chain a `$LIBRAGENT_WAKE_URL` POST, end the turn |
| Events from a system you do not control (something arrives, changes, fires) | **B** — hook/watcher POSTs to `$LIBRAGENT_WAKE_URL`; the woken session handles the event |

### Is wake available here?

The only environment check: is `$LIBRAGENT_WAKE_URL` set? The variable exists only in
`runShell` / `spawnProcess` environments, so test it there with
`[ -n "$LIBRAGENT_WAKE_URL" ]` (never print it).

- **Set** → A2 and B work as written.
- **Unset** → nothing can wake this session after the turn ends:
  - Process: block on `workspace__waitForProcess` (A).
  - External event: see "If `$LIBRAGENT_WAKE_URL` is unset" in B, or tell the user wake is not
    available in this environment.

## vs `loop` / `schedule` (hard split)

| Signal | Skill |
| --- | --- |
| Process exit, job/ticket done, arriving event, webhook, watcher, hook | **`call-me-back`** (this skill) |
| Relative delay / wall-clock / checking on a time interval | **`loop`** |
| Permanent app-wide cron | **`schedule`** |

If the user names a **command, job, ticket, or external system / event source**, use this
skill. If they only name a **clock time or interval**, use `loop`.

## Out of scope

- "Remind me in 10 minutes" / "내일 9시에" → `loop`
- Always-on global cron with no session binding → `schedule`
- Immediate child-session work with no wait → `delegate`

## Workflow

### A. Process this session started — wait in this turn

`workspace__waitForProcess` only works for a process **this session** started with
`runShell` / `spawnProcess` and got a `processId` for (in a managed Docker session that
includes processes running in the container). It cannot wait for anything else.

1. Start (or reuse) a background process:
   `workspace__spawnProcess` (or a sync tool that handed off a `processId`).
2. Block on completion — do **not** poll with `loop`:
   `workspace__waitForProcess(processId=..., timeout=...)`
   (default timeout is 30 seconds; give a timeout that fits the job.)
3. On finish, read output if needed:
   `workspace__readProcessOutput(processId=...)`
4. Continue the user task with the result (pass/fail, logs, next steps).

Prefer one `waitForProcess` with a fitting timeout over repeated short waits or clock callbacks.

### A2. Process this session started — end the turn, get woken

Requires `$LIBRAGENT_WAKE_URL` (see "Is wake available here?"); if unset, use A.

For jobs that run many minutes (long builds, training) with nothing else to do meanwhile,
do not hold the turn in `waitForProcess`. `runShell` / `spawnProcess` export
`$LIBRAGENT_WAKE_URL`, a tokenized wake URL for **this** session only.

1. Start the job with an unconditional callback (fires on success **and** failure):
   `long_cmd > job.log 2>&1; curl -s -X POST "$LIBRAGENT_WAKE_URL" -H 'Content-Type: application/json' -d "{\"exitCode\":$?,\"logPath\":\"job.log\"}"`
   via `workspace__spawnProcess`.
2. Tell the user it is running, then **end the turn**.
3. You are woken by a `[wake callback]` message (see "Wake message"). Read the log and
   continue. If the session is busy, the callback is queued, not dropped.

- Never `echo` / print `$LIBRAGENT_WAKE_URL` or paste its value into files or replies.
- No `curl` in the image: POST with an HTTP client it has (for example `python3`).
- Contract (modes, exposure, security): `docs/architecture/session-wake-contract.md`.
- The token is revoked when the session is terminated or deleted, and does not
  survive an app restart; late callbacks get `401`. If no callback came, check
  `job.log` yourself.

### B. External events (hooks / webhooks / watchers)

**Principle: the hook only wakes this session. The woken session decides and responds.**

**Success** = a message lands in **this** LibrAgent session and the session handles the
event. Stdout from an external gateway/hook process is **not** resume.

#### What a hook / watcher may do

A hook, webhook command, or watcher process does exactly three things:

1. Detect the event.
2. Put the event content into the payload: `summary` (text) or a file referenced by `logPath`.
3. POST to `$LIBRAGENT_WAKE_URL`.

Summarizing, deciding, or sending a reply from inside the hook/watcher is an anti-pattern:
that work happens in the woken session, where the user's task and context live.
A long-running watcher that POSTs once per event is fine — that is how repeated events
are received.

#### Contract (product-agnostic)

Do **not** add per-product recipes to this skill — read that system's own hook docs, then
wire its payload here.

Every wake goes through `$LIBRAGENT_WAKE_URL`, the tokenized wake URL for **this** session.
Point the hook, webhook or `--exec` command at it. Do not invent a path, do not use the
main Session API `/messages`, and do not leave `--exec` as `echo` / file-append only.

- The variable exists only in `runShell` / `spawnProcess` environments. Expand it when you
  register the callback (double-quoted), so the stored hook carries the value. Never print it.
- Body: optional JSON (`status`, `exitCode`, `summary`, `logPath`); an empty body also wakes.
- For events, set `status` yourself (for example `"status":"event"`) and put the event
  content in `summary` (up to 2000 characters) or write it to a file and send its path in
  `logPath`.

#### Two phases

1. **Register** the hook / start the watcher once (that system's register API, or a
   `workspace__spawnProcess` watcher). Tell the user it is set up, then end the turn.
2. **When woken** (this session receives the `[wake callback]` message): read the event
   from `summary` / `logPath`, then decide and respond here.

Do not treat "callback registered" as done unless wake/inject is wired.
Do not assume work done only inside the external callback will appear in this chat.

#### If `$LIBRAGENT_WAKE_URL` is unset

Nothing can wake the session after the turn ends. Either tell the user wake is not available
in this environment, or:

1. Start a watcher with `workspace__spawnProcess` that exits after the **first** event and
   writes it to a file.
2. Block on `workspace__waitForProcess` with a long timeout (minutes, not the 30-second
   default); if it times out, wait again.
3. Read the event file and handle it in this session.

This handles one event per wait. If the user asked for an ongoing subscription, tell them
about this limit.

#### Anti-patterns

- A hook/watcher that summarizes, decides, or replies by itself instead of waking this session
- External callback that only logs, `echo`s, or appends a file — **no session inject**
- Registering `--exec` / a webhook without the `$LIBRAGENT_WAKE_URL` `POST`
- "Callback runs outside this process so it cannot reach this chat" — **false**; inject is the reach
- With `$LIBRAGENT_WAKE_URL` set: blocking long-poll / foreground watchers when a
  non-blocking register-callback API exists
- Faking completion with `scheduled_task__scheduleCallback` / `loop` delays

#### If the system only offers coarse polling

Prefer its native wait/watch over LibrAgent clock loops, and keep poll intervals
honest in the reply. Prefer inject/wake whenever a callback hook exists.

### Wake message

Every wake — process or external event — arrives as one message in this format:

```text
[wake callback] Background process finished: status=<status>
exitCode: <exitCode>      (if sent)
logPath: <logPath>        (if sent)
summary: <summary>        (if sent)
```

The first line always says "Background process finished", also for external events.
`status` is your `status` value as sent (up to 32 characters, for example `event`); without
it, it is derived from `exitCode` (`0` → `success`, other → `failure`) or is `unknown`.

### C. Ambiguous "끝나면 알려줘"

- Named process/command/job/ticket/system or event source → this skill
- Only a duration, clock time, or interval → `loop`
- Still unclear → ask once: time-based reminder, or wait for a specific completion/event?

## Guardrails

- Do not use `scheduled_task__scheduleCallback` as a stand-in for job completion.
- Do not busy-poll via many tiny `loop` delays or LLM turn spam.
- Do not invent `processId` values; only use ids returned by workspace tools.
- Do not treat this skill as user push-notification / telephony.
- Keep session context: completion should resume **this** chat unless the user
  explicitly asked for a global scheduled worker (`schedule`).
- Do not invent per-product command catalogs in this skill; reuse the inject contract.

## Related skills

- **`loop`** — clock-based session reminders and recurring checks
- **`schedule`** — global cron automation
- **`delegate`** — spawn work in a child session now
- **`libragent-harness-reference`** — HTTP inject path / session id facts
- **`teamwork` / `org`** — multi-agent constitution; not required for a single wait
