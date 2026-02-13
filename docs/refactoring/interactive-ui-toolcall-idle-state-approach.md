# Interactive UI Tool Call Idle-State Refactoring (Implemented)

## Status

This document reflects the **actual implementation** on branch `dev/0.4.0`.

The prior version described a design proposal. That proposal is now partially replaced by implemented behavior.

---

## What Was Implemented

### 1) Message-boundary cancel handling

- `cancel_pending` is now treated as a **cancel intent**.
- If a tool-call batch is in progress (`pending_execution.is_some()`), `cancel_workflow` does not force immediate stop.
- The workflow consumes cancel at **message boundary** (after the current message's full tool-call batch completes).

Implementation points:

- `agent/workflow.rs` (`cancel_workflow`):
  - sets `cancel_pending = true`
  - defers stop when a pending tool execution exists
  - immediate stop only when no in-flight batch exists
- `agent/workflow.rs` (`continue_workflow_after_tool`):
  - checks `cancel_pending` after all expected tool results are collected
  - consumes flag and transitions to idle at message boundary

### 2) Message/tool-call integrity guard

`PendingToolExecution` now carries message-scoped ownership and idempotency data:

- `message_id`
- `expected_tool_call_ids`
- `completed_tool_call_ids`

Tool result handling rejects:

- **Stale** tool_call_id (not expected for current message)
- **Duplicate** tool_call_id (already completed)

Implementation points:

- `agent/state.rs`: extended `PendingToolExecution`
- `agent/llm.rs`: initializes expected set when scheduling tool execution
- `agent/tools.rs`: enforces stale/duplicate checks

### 3) API semantic alignment

- `agent_cancel_workflow` response message updated to **"cancel requested"** semantics.
- This matches deferred cancellation behavior during in-flight message execution.

Implementation point:

- `commands/agent_commands.rs`

### 4) Pending event cleanup simplification

- `PendingEvent::CancelRequested` removed.
- `PendingEventManager` now tracks message events only.

Implementation points:

- `agent/state.rs`
- `agent/workflow.rs` (removed obsolete enqueue)

---

## Current Runtime Semantics

### Cancel behavior

1. User presses cancel.
2. Backend sets `cancel_pending = true`.
3. If no in-flight tool batch: stop immediately.
4. If in-flight tool batch exists: finish current message's tool-call batch.
5. At message boundary, consume cancel and stop.

### Tool result integrity

For current pending message execution:

- Accept only expected tool_call IDs.
- Ignore duplicate tool_call IDs.
- Complete message batch when all expected IDs are completed.

---

## Tests Added

### `agent/tools.rs`

- `test_classify_tool_result_accepts_expected_unseen_id`
- `test_classify_tool_result_rejects_stale_id`
- `test_classify_tool_result_rejects_duplicate_id`

### `agent/workflow.rs`

- `test_classify_cancel_strategy_defers_when_pending_execution_exists`
- `test_classify_cancel_strategy_stops_immediately_without_pending_execution`
- `test_should_consume_cancel_at_message_boundary_only_when_pending_flag_set`

---

## What Is Not Yet Implemented

The following stronger ownership checks are still future work:

- `run_id` based validation
- `parent_message_id` correlation validation
- explicit typed error categories for invalid continuation payloads
- end-to-end integration test covering deferred cancel through full event flow

---

## Practical Outcome

- Removed brittle tool-name allowlist pattern from the refactoring direction.
- Cancel is now aligned with message-level integrity.
- Tool result processing is message-scoped, idempotent, and safer against stale/duplicate replies.
