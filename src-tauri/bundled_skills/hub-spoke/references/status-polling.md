# Hub Status Polling

Hub monitors spokes without loading full conversation logs.

## Polling workflow

1. After `agent__startSession(..., waitForResult=false)`, store `sessionId`.
2. Poll with `agent__checkSession(sessionId)` on an interval or before next dispatch.
3. Request **summary fields only**: status, last message snippet, output file paths.

## Status decisions

| Status | Hub action |
| --- | --- |
| running | wait or work on other spokes |
| completed | collect artifacts, merge or route to dependent spoke |
| failed | retry once, reassign, or escalate to user |
| stuck (no progress N checks) | `agent__messageToSession` nudge or stop |

## Anti-patterns

- Do not `history__readSession` on every spoke each poll — context explosion.
- Do not spawn duplicate spokes for the same task without stopping the first.

## Completion handoff to synthesis

When all spokes complete, Hub collects:

- Primary artifact paths (files)
- One-paragraph outcome per spoke
- Open risks or blockers

Then runs integration step (tests, merge doc, user summary).
