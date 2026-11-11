---
name: libr-e2e-verify
description: >
  End-to-end verification of a running LibrAgent from an external agent (Claude Code,
  Cursor, or a LibrAgent session) over the HTTP Session API and the debug E2E control
  plane. Use when asked to "E2E 검증", "verify this build E2E", "run the smoke matrix",
  "smoke test LibrAgent", or when executing the `[auto]` items of an `auto:e2e` ticket
  checklist with per-item pass/fail evidence. One invocation runs
  health → create session → message → control-plane check → transcript check → cleanup
  and prints a pass/fail table. No new server code; the skill never starts or stops
  LibrAgent. Not for spawning a session alone (libr-delegate), builtin-tool improvement
  loops (libr-tool-e2e-harness-loop), Harbor benchmarks (bench), or diff-driven
  release-to-HEAD campaigns (release-head-self-e2e).
---

# Libr E2E Verify

Verify a running LibrAgent end to end over HTTP only. The interfaces already exist;
this skill is the knowledge and the pass/fail discipline:

**preconditions → session lifecycle → blocking interactions → evidence → classification → cleanup**

## When to use

- Verify a built/installed app after a release or a merge batch (smoke matrix).
- Execute `[auto]` items of open `auto:e2e` issue checklists with one evidence pointer per item.
- Any assertion an external harness can make over HTTP without touching the app UI.

## Preconditions

| Check | How | Meaning |
| --- | --- | --- |
| Health gate | `GET /api/health` → `status: ok` | Proceed. If unreachable, abort: the app is not up; the harness owns the app lifecycle, never start it here. |
| Control plane | `GET /api/e2e-control/status` with header `X-LibrAgent-E2E-Control: <cap>`, where `<cap>` is the content of `e2e_control.cap` under `LIBRAGENT_DATA_DIR` | `200` = debug build with `LIBRAGENT_E2E_CONTROL=1` and an explicit `LIBRAGENT_DATA_DIR`. `404`/`405` = absent (release build, debug without opt-in, or wrong/missing header). |
| Fallback | Control plane absent → run with `executionMode: yolo` (standard tools) or `unsafe` (also hard-approval tools) | Report `control plane absent — yolo/unsafe fallback`, do not fail the run. |

- The control plane is debug-only and loopback-only. Never expect it on a release build.
- `Authorization: Bearer <key>` on the Session API is configuration-dependent; send it only when the user provides an API key.

## Session lifecycle

```
POST /api/sessions            {assistantId, executionMode, workspacePath, request?}   # omit request → idle session
POST /api/sessions/:id/messages  {content, source}                                    # busy → queued
GET  /api/sessions/:id                                                             # poll status: idle | busy | paused | error
GET  /api/sessions/:id/children                                                    # only when delegation is expected
POST /api/sessions/:id/resume                                                      # reload paused/crashed in place (also the "session reload" path for process-retention checks)
POST /api/sessions/:id/terminate
DELETE /api/sessions/:id
```

- Always pass an absolute `workspacePath`; use `unsafe` only in a scratch directory, never the repo or the user workspace.
- `assistantId` is required; prefer the Master Mind assistant for unattended runs.

## Blocking interactions over HTTP (debug control plane)

| Need | Call |
| --- | --- |
| See what blocks a session | `GET /api/e2e-control/sessions/:ref/interactions` → pending user prompt + pending approvals |
| Answer or cancel a `ui__askUser` / storeAs prompt | `POST /api/e2e-control/sessions/:ref/user-prompts` with `{"requestId": "...", "action": "answer"|"cancel", "value": "..."}` |
| Approve or deny a tool approval | `POST /api/e2e-control/sessions/:ref/tool-approvals` with `{"toolCallId": "...", "approved": true|false}` |

Without the control plane, prompts and approvals block until a human answers or
`yolo`/`unsafe` auto-approves them.

## Evidence and results

- Extract the transcript **before** cleanup: `GET /api/sessions/:id/messages?limit=500`.
- One result per item: `pass` / `fail` / `blocked`, plus one evidence pointer (session id,
  transcript excerpt, process output, or log grep line).
- `fail` links a bug issue; `blocked` names its blocker.
- Never print secret values in evidence; grep for presence/masking, not the value. Dummy
  secrets only (for example `dummy-token-abcd`).

## Item classification

- `[auto]` — observable over HTTP: session status, transcript content, tool output, log greps.
- `[manual]` — visual/rendering assertions (masked input, scroll, viewport), unless a screenshot + vision step is added.
- `[manual]` — OS-level steps (sudo, OS keychain), unless the runner provides them.

## Cleanup discipline

- Terminate then delete every one-shot session; never leave orphan sessions behind.
- `--keep-session` only while debugging a failed run.

## Smoke matrix (one invocation)

```
node .agents/skills/libr-e2e-verify/scripts/smoke_matrix.cjs --assistant <config-id> \
  [--base-url http://localhost:3030] [--workspace <scratch-dir>] [--api-token <key>] \
  [--execution-mode yolo|unsafe] [--timeout 180] [--keep-session]
```

Runs health → create → message → poll → control-plane check (reads interactions; approves
any pending tool approval) → transcript check → terminate + delete, and prints one
pass/fail table. Exit `0` only when every step passes; a release build passes the
control-plane step as `absent (yolo/unsafe fallback)`, not as a failure.
