# Workflow Settlement

## Problem

Delegated sub-sessions and parent `checkSession(wait=true)` callers can wake up as
soon as a child session becomes terminal (`Idle` or `Error`). If the terminal
assistant message is only present in the in-memory cache, or if `Idle` is
published before pending work is drained, parents can observe:

- an empty or stale `result` from `checkSession`
- a session that appears finished while another LLM turn is still required

The old completion paths updated status and emitted events from several
call sites, which made durability and race handling easy to miss.

## Solution: Centralized Settlement Gate

All natural-completion and workflow-error finalization paths now converge on
`src-tauri/src/agent/workflow/finish.rs`.

### Core Functions

| Function                              | Purpose                                                                             |
| ------------------------------------- | ----------------------------------------------------------------------------------- |
| `persist_terminal_assistant_sync`     | Durably upsert the terminal assistant row before any terminal status is exposed     |
| `continue_workflow_if_pending_events` | Drain `pending_events` and request another LLM turn when work arrived during finish |
| `settle_before_terminal_transition`   | Persist + first pending re-check                                                    |
| `settle_session_and_go_idle`          | Success path → `Idle` + `WorkflowCompleted`                                         |
| `settle_session_and_finalize_error`   | Error path → `Error` + `WorkflowError`                                              |

Production callers still use the public wrappers. Tests can inject a custom
`AgentEventDispatcher` through the `*_with_dispatcher` variants.

### Success Path

```text
LLM natural completion
  → persist_terminal_assistant_sync
  → continue_workflow_if_pending_events
      → if pending: request_llm_completion_with_recovery, return Ok(true)
  → continue_workflow_if_pending_events   # finish-window guard
      → if pending: request_llm_completion_with_recovery, return Ok(true)
  → update_session_status(..., Idle)
  → emit WorkflowCompleted
```

`settle_session_and_go_idle` returns:

- `Ok(true)` when pending work restarted the workflow
- `Ok(false)` when the session settled to `Idle`

### Error Path

```text
LLM / workflow failure
  → persist_terminal_assistant_sync
  → update_session_status(..., Error)
  → emit WorkflowError
```

The error settlement path intentionally does **not** re-check `pending_events`.
Doing so would recurse back into `handle_llm_error_with_outcome` and create an
async recursion cycle.

### Fail-Closed Persist Errors

If terminal persistence fails:

1. The technical error is logged server-side only
2. The caller receives a user-facing message with no DB internals
3. The session transitions to `Error`
4. A `WorkflowError` event with code `TERMINAL_PERSIST_FAILED` is emitted when possible

This keeps parent `checkSession` and the UI aligned even when persistence fails.

## Race Mitigation

### Finish-Window TOCTOU

A message can be queued into `pending_events` after the first pending scan inside
`settle_before_terminal_transition` but before `Idle` is written. The second
`continue_workflow_if_pending_events` call immediately before the `Idle`
transition closes that window.

### What We Did Not Add

- No new `Completing` / `Settled` state machine
- No separate `session_messages` table
- No parent error propagation helper
- No pending re-check on the error settlement path

Those were rejected as unnecessary complexity or recursion hazards.

## Event Emission and UI Consistency

Terminal status changes always go through
`update_session_status_with_dispatcher`, which emits `StatusChanged` before the
workflow-level event.

| Event               | When it fires                      | UI role                        |
| ------------------- | ---------------------------------- | ------------------------------ |
| `StatusChanged`     | Every status transition            | Ground truth for session state |
| `WorkflowCompleted` | Successful settlement              | Completion reason for chat UI  |
| `WorkflowError`     | Failed workflow or persist failure | Structured recoverable error   |

If `WorkflowCompleted` or `WorkflowError` emission fails after the status update,
settlement returns a user-facing error and logs that `StatusChanged` should
already have reached the UI. Callers should treat the DB/session status as
authoritative and refresh if needed.

## Paths That Bypass Settlement

These flows intentionally do not use the settlement gate:

- user cancel (`cancel.rs`)
- session reset / provisioning
- compaction-only or infrastructure transitions

Only workflow completion paths that expose a terminal assistant result to
delegation callers use settlement.

## Testing

Integration coverage lives in
`src-tauri/tests/integration/workflow_settlement_durability_tests.rs`:

- success settlement to `Idle`
- error settlement to `Error`
- cache-only terminal message durability before terminal status
- pending-event detection for restart / finish-window guards
- user-facing persist failure propagation
- workflow event emit failure after terminal status is already durable

Unit coverage lives in `src-tauri/src/agent/workflow/finish.rs` for helper
logic, event emission failure handling, and `WorkflowError` emission on persist
failure.

## Related Documents

- [agent-workflow-architecture.md](./agent-workflow-architecture.md) — overall dual-backend workflow model
- [session-cancel-isolation.md](./session-cancel-isolation.md) — parent cancel while waiting on child sessions
