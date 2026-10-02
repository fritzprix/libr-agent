---
name: libr-delegate
description: >
  Delegate work to a running LibrAgent desktop/headless instance via the local
  HTTP Session API (`POST /api/sessions`, messages, status, children, resume).
  Use from Cursor (or other external harnesses) when LibrAgent is up and you
  need a LibrAgent session to run a task — optionally with `parentSessionId`
  lineage. Triggers: Session API, /api/sessions, parentSessionId, curl session,
  HTTP delegate to LibrAgent, "LibrAgent에 위임", "세션 API로 자식 띄워",
  Harbor/bench spawn. NEVER for LibrAgent-internal `agent__spawnSession` flows
  (that is the in-app `delegate` skill). NEVER invent MCP tool names here.
---

# Libr Delegate (Cursor → LibrAgent Session API)

Spawn and steer **LibrAgent** sessions over HTTP from Cursor. LibrAgent must
already be running (desktop or headless). This skill does **not** ship into
LibrAgent `bundled_skills`.

## vs in-app `delegate`

| Caller | Mechanism |
| --- | --- |
| Cursor / scripts / Harbor | **This skill** → HTTP `/api/sessions` |
| Inside a LibrAgent agent turn | In-app **`delegate`** → `agent__*` tools |

## Base URL

```bash
PORT=$(cat ~/.libragent/http_port 2>/dev/null || echo 3030)
BASE="http://127.0.0.1:${PORT}"
```

Port file is written when LibrAgent’s HTTP server binds; fallback is `3030`.

## Workflow

1. Confirm LibrAgent is up — `GET $BASE/api/health` (or assistants list)
2. Resolve `assistantId` — `GET $BASE/api/assistants`
3. Write a self-contained handoff (LibrAgent child does **not** see Cursor
   chat, parent workspace instructions, or workspace-local skills unless you
   pass an absolute `workspacePath`)
4. Create — `POST $BASE/api/sessions` (include `request` or create idle)
5. If idle — `POST $BASE/api/sessions/:id/messages` with `{"content":"..."}`
6. Poll — `GET $BASE/api/sessions/:id` until not Busy/Provisioning
7. Read result — `GET $BASE/api/sessions/:id/messages`
8. On Paused after crash — `POST …/resume`, then poll again
9. Optional lineage — set `parentSessionId`; list with `GET …/children`
10. Cleanup — `POST …/terminate` and/or `DELETE …/sessions/:id`

Curl recipes: `references/session-api.md`. Handoff / isolation:
`references/handoff.md`.

## Create payload essentials

```json
{
  "assistantId": "<from GET /api/assistants>",
  "request": "<self-contained task + deliverable format>",
  "workspacePath": "/absolute/path/optional",
  "parentSessionId": "<optional exact parent session id>",
  "executionMode": "normal | yolo | unsafe",
  "name": "optional label"
}
```

- Blank/omit `request` → idle (no LLM turn until messages POST)
- Unattended automation: prefer `yolo`; use `unsafe` only when hard-approval
  tools must also auto-run
- Child inherits non-`normal` parent `executionMode` when omitted on create

## Polling rules

| Status | Action |
| --- | --- |
| `Busy` / `Provisioning` | Wait with backoff; do not spam |
| `Idle` (after work) | Read messages; treat latest assistant text as result |
| `Paused` | `POST …/resume` once, then poll |
| `Error` | Fail the handoff; do not claim success from partial prose |

Follow-ups: another `POST …/messages` (queued if still Busy).

## Isolation (critical)

The LibrAgent session is not a clone of this Cursor chat:

- Default: LibrAgent session workspace, not the Cursor CWD unless
  `workspacePath` is set to that absolute path
- Put objective, scope, paths, and output format in `request` / message body
- Require deliverables in the session’s **final assistant text**
- Do not assume Cursor files, notes, or tools are visible inside LibrAgent

## Anti-patterns

- Calling nonexistent MCP tools (`spawnSession`, `session_api__*`) from Cursor
- Echo-only stubs instead of real `POST …/messages`
- Rapid fixed-interval polls with no backoff
- Treating Paused / empty history / Error as success
- Putting this skill under `src-tauri/bundled_skills` (Cursor-only)

## Related repo docs

- HTTP API: `docs/api/http_api.md`
- In-app sub-agents (product): `docs/user/guides/sub-agents.md`
