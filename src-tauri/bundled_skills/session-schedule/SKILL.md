---
name: session-schedule
description: PRIMARY and DEFAULT skill for user-facing schedules, reminders, and callbacks. Use when the user asks to remind, notify, follow up, or run recurring checks inside the active conversation (via scheduleCallback). Always default to this skill over schedule for in-session requests. Only escalate to global schedule when explicitly asked for permanent, app-wide background automation.
---

# Session Schedule (PRIMARY for user-facing schedules)

Session schedules inject a message into **the current session** at a future time. They preserve the active conversation context, history, and workspace, surviving tab switches and app restarts while staying bound to this session's lifecycle.

The MCP tool is `scheduleCallback` (or `scheduled_task__scheduleCallback`). The skill name is `session-schedule`.

## Core Policy: Session-First by Default

When a user asks to schedule a reminder or task inside an active conversation (e.g., "remind me in 5 minutes", "내일 9시에 알려줘", "이 작업 끝나면 리마인드해줘"), they almost always expect it **in this conversation's context**. Wrong skill choice is usually selecting global `schedule` / `createScheduledTask`, which drops session context and may force an unnecessary `assistantId` question.

**Always default to `session-schedule`** for:
- "Remind me in X minutes / hours / days" ("X분 뒤에 알려줘")
- "Tomorrow at 9" / "내일 9시에 알려줘" when continuing this chat
- "Follow up on this later" ("나중에 확인해줘")
- "Notify me when / after..." ("끝나면 알려줘")
- In-session recurring checks tied to this work ("매일 아침 9시에 이 작업 현황 알려줘")
- Any relative delay or wall-clock reminder that should reuse this chat's history

### When to escalate to global `schedule`:
Escalate to global `schedule` (`createScheduledTask`) **ONLY** when:
- The user explicitly asks for an independent, permanent, or app-wide schedule (e.g., "even if I close this chat", "permanently in the background", "전역 백그라운드 작업으로 등록해줘").
- The automation must run as a standalone assistant task without any session context.
- It is a multi-agent coordination trigger or scheduled teamwork run.

### Handling Ambiguous Requests:
If intent is ambiguous (e.g., "매일 9시에 주가 체크해서 알려줘" with no global/permanent wording):
1. **Proceed with `session-schedule` immediately** — do not ask first by default.
2. **Ask only when** the request looks like always-on automation that should survive this chat (recurring wall-clock work with no tie to this conversation's task/context). Then explain once:
   - **Session Schedule (default)**: injects into this chat; invalidated if the session is deleted.
   - **Global Scheduled Task**: independent app-wide cron; survives closing/deleting this session; no session history.

## Routing decision

```text
Should the run stay bound to the current session?
  Yes                  -> session-schedule (scheduleCallback) [DEFAULT]
  Unclear / Ambiguous  -> session-schedule immediately [DEFAULT]
                          ask only if always-on / session-surviving intent is plausible
  No (Explicit global) -> schedule (createScheduledTask)
```

Both one-shot (`delaySeconds`) and recurring (`cronExpression`) schedules are supported here. Pick the timing mode, not a different skill.

## Workflow

### 1. Choose timing mode

Provide **exactly one** of:

- **`delaySeconds`** — one-shot delay (1–86400 seconds). Example: 300 for "check back in 5 minutes".
- **`cronExpression`** — recurring session schedule. Example: `0 9 * * *` for every day at 09:00 local time.

Do not pass both. Do not pass neither.

### 2. Create the schedule

```text
scheduleCallback(
  message="...",           // required instruction injected when the callback fires
  name="...",              // optional label for Planning panel / lists
  delaySeconds=300         // OR cronExpression="0 9 * * *"
)
```

Requirements:

- Must run from an **active session**. The tool binds to the current session automatically.
- `message` is injected when the schedule fires. Make it self-contained and descriptive enough for the agent to act effectively when triggered.
- `assistantId` is NOT required; the backend automatically resolves it from the current session.

### 3. Manage existing session schedules

After creation, use the returned task ID:

- `getScheduledTask(taskId)` — inspect timing, message, and enabled state
- `toggleScheduledTask(taskId, enabled=false)` — pause without deleting (ambient SC clears while paused)
- `deleteScheduledTask(taskId)` — cancel / remove it entirely

The user can also manage and cancel schedules directly from the session Planning panel.

### 4. Set expectations honestly

- One-shot schedules are deleted after they fire.
- If the session is deleted, session schedules are removed; they do not create a replacement session.
- Recurring session schedules keep firing until paused, deleted, or the session is removed.
- Injected messages appear in the chat stream when the schedule fires.

## Guardrails

- Do not use `createScheduledTask` for in-session reminders or delays; it creates global tasks and requires `assistantId` plus cron.
- Do not ask the user for an `assistantId` when setting a reminder; `scheduleCallback` automatically binds to the current session.
- Do not require `.libragent/teamwork.json` or teamwork scaffold files.
- Prefer `delaySeconds` for relative delays; use cron when the user names a wall-clock recurrence.
- When scheduling multiple callbacks, give each a distinct `name` when possible so the Planning panel stays readable.

## Related skills

- **`schedule`** — global cron tasks that survive beyond the current session (independent background workers)
- **`delegate`** — spawn a child session now, not a future injection into this one
- **`teamwork`** — only when building multi-agent workspace constitution; not needed for simple session schedules
