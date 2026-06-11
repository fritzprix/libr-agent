---
name: session-schedule
description: Schedule one-shot delays or recurring runs bound to the current agent session in LibrAgent. Use delaySeconds for a single future injection, cronExpression for session-scoped recurrence, or when the user asks to continue, remind, or follow up later in this conversation. Does not require teamwork or task groups. For app-wide automation that outlives the session, use schedule instead.
---

# Session Schedule

Session schedules inject a message into **the current session** at a future time. They survive tab switches and app restarts, but they are tied to this session's lifecycle.

The MCP tool is `scheduleCallback`. The skill name is `session-schedule`.

## When to use this skill

Use `session-schedule` when:

- the user wants to continue **this conversation** later ("check in 5 minutes", "resume tomorrow morning here")
- a reminder or follow-up must reuse the current session context and history
- the user schedules multiple callbacks in the same thread (N:1 on one session)
- recurrence should stay inside the current session (hourly summary of **this** thread)

Stop and use `schedule` when:

- automation should outlive the session or target a different assistant explicitly
- the user wants app-wide cron jobs visible in Scheduled Tasks settings
- teamwork needs grouped global automation loops with `groupName` / `groupId`

Teamwork scaffolding is **not** required.

## Routing decision

```text
Should the run stay bound to the current session?
  Yes -> session-schedule (scheduleCallback)
  No  -> schedule (createScheduledTask)
```

Both one-shot and recurring schedules are supported here. Pick the timing mode, not a different skill.

## Workflow

### 1. Choose timing mode

Provide **exactly one** of:

- **`delaySeconds`** — one-shot delay (1–86400 seconds). Example: 300 for "check back in 5 minutes".
- **`cronExpression`** — recurring session schedule. Example: `0 9 * * *` for every day at 09:00 local time.

Do not pass both. Do not pass neither.

### 2. Create the schedule

```text
scheduleCallback(
  message="...",
  name="...",              // optional label for Planning panel / lists
  delaySeconds=300         // OR cronExpression="0 9 * * *"
)
```

Requirements:

- Must run from an **active session**. The tool binds to the current session automatically.
- `message` is injected when the schedule fires. Make it self-contained enough for the agent to act without guessing.
- `assistantId` is not required; the backend resolves it from the current session.

### 3. Manage existing session schedules

After creation, use the returned task ID:

- `getScheduledTask(taskId)` — inspect timing, message, and enabled state
- `toggleScheduledTask(taskId, enabled=false)` — cancel or pause before it fires
- `deleteScheduledTask(taskId)` — remove it entirely

The user can also cancel from the session Planning panel Schedules section.

### 4. Set expectations honestly

- One-shot schedules disable themselves after firing.
- If the session is deleted, session schedules are invalidated; they do not create a replacement session.
- Recurring session schedules keep firing until paused, deleted, or the session is removed.
- Injected messages appear in the chat stream when the schedule fires.

## Guardrails

- Do not use `createScheduledTask` for in-session delays; it creates global tasks and requires `assistantId` plus cron.
- Do not use `groupName` or `groupId`; session schedules do not support groups.
- Do not require `.libragent/teamwork.json` or teamwork scaffold files.
- Prefer `delaySeconds` for relative delays; use cron when the user names a wall-clock recurrence.
- When scheduling multiple callbacks, give each a distinct `name` when possible so the Planning panel stays readable.

## Related skills

- **`schedule`** — global cron tasks, optional groups, survives beyond the current session
- **`delegate`** — spawn a child session now, not a future injection into this one
- **`teamwork`** — only when building multi-agent workspace constitution; not needed for simple session schedules
