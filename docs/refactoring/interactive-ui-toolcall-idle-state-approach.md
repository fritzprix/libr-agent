# Design Discussion: UI Tool Call Rejection in Idle State

## Why this document

This document captures the reasoning behind the current rejection behavior and proposes a safer workflow model for interactive UI tool continuation (e.g., `executePendingShell`).

The goal is to align on architecture before implementing code changes.

---

## Current behavior (observed)

### Runtime sequence

1. Agent executes a tool that returns UIResource (interactive step 1)
2. Workflow loop detects UI interaction and **stops**
3. Session status transitions to **Idle**
4. User submits UI input, frontend injects tool call (`executePendingShell`)
5. Backend receives it via `agent_handle_llm_response`
6. `llm::handle_llm_response` rejects because status is not `Busy`
7. User sees error: `Workflow was cancelled`

### Why this guard exists

The guard (`status == Busy`) was introduced as a defensive mechanism to block stale/delayed frontend responses after a workflow is cancelled or already ended.

Historically this made sense for standard request/response turns:

- Busy means workflow owns the turn
- Idle means no active run should accept LLM response payloads

But the 2-step interactive pattern changed semantics:

- Idle is now expected between step 1 and step 2
- Step 2 is valid continuation, not stale traffic

---

## Root mismatch

**Current invariant:** only `Busy` sessions may process assistant/tool-call injections.

**Interactive invariant:** a validated UI continuation may legitimately arrive while `Idle`.

These invariants conflict.

---

## Proposed approach

## A. Narrow acceptance rule (recommended)

Allow Idle-state acceptance **only** for validated interactive continuation tool calls.

### Acceptance conditions

A tool-call injection is accepted in Idle only if all are true:

1. Message contains exactly one tool call
2. Tool name is one of:
   - `builtin_workspace__executePendingShell`
   - `builtin_workspace__cancelPendingExecution`
3. Arguments include `executionId` (or temporary fallback key)
4. Referenced pending execution exists in `WorkspaceServer.pending_executions`
5. Pending execution belongs to same `session_id`
6. Pending execution has not expired

If any check fails, keep existing rejection behavior.

### State transition

When accepted in Idle:

`Idle -> Busy -> execute tool -> continue normal workflow policy`

On completion:

- If tool output is terminal and no further model turn needed: `Busy -> Idle`
- If recursion should continue: proceed with standard workflow continuation

---

## B. Where to implement (minimal-scope)

### Primary gate (preferred)

In command entrypoint or session-manager boundary before `llm::handle_llm_response` strict Busy guard:

- detect “interactive continuation candidate”
- run validation
- if valid, transition status to Busy and proceed

### Keep strict guard unchanged for normal paths

Avoid weakening global safety checks in `llm::handle_llm_response` for general traffic.

---

## C. Why this is safer than removing Busy check

### Do NOT do this

- Removing `status != Busy` checks globally
- Accepting all Idle assistant messages

### Risks avoided by narrow gate

- stale replay acceptance
- race-induced duplicate tool execution
- cross-session execution leakage
- unintended assistant-injected calls during idle UI periods

---

## D. Alternative designs considered

## Option 1: New dedicated command for UI tool continuation

Example: `agent_handle_ui_tool_call(session_id, tool_name, args)`

Pros:

- explicit protocol split
- no ambiguity with LLM response path

Cons:

- larger frontend/backend API change
- migration overhead

## Option 2: Reuse `agent_inject_messages(trigger_workflow=true)`

Pros:

- uses existing path

Cons:

- still must bypass Busy-only gate safely
- less explicit semantics

## Option 3: Always keep session Busy during UI wait

Pros:

- no Idle continuation issue

Cons:

- poor UX semantics (looks running while waiting user)
- higher chance of confusing cancellation/queue logic

**Recommended now:** Option A (narrow acceptance rule)

---

## E. Contract proposal

### Backend contract

If a UI interactive continuation is valid:

- backend must accept while Idle
- backend must perform atomic state transition to Busy before processing

If invalid:

- return explicit error category (`not_found`, `expired`, `session_mismatch`, `invalid_payload`)
- keep session Idle

### Frontend contract

UI renderer continues sending tool payload through existing path.
No UX-level retry loops unless backend returns retryable category.

---

## F. Test plan for this change

### Unit

1. Idle + valid `executePendingShell` => accepted and transitions Busy
2. Idle + invalid tool name => rejected
3. Idle + missing executionId => rejected
4. Idle + expired pending execution => rejected
5. Idle + wrong session ownership => rejected
6. Busy + normal behavior unchanged

### Integration

1. interactive step1 returns UIResource, status becomes Idle
2. UI submit triggers step2
3. step2 executes successfully
4. no `Workflow was cancelled` error emitted

### Regression

1. stale assistant response after explicit cancel still rejected
2. non-interactive idle tool injection still rejected

---

## G. Open questions for discussion

1. Should accepted Idle continuation always emit a dedicated event (e.g., `WorkflowResumedForUIAction`) for clearer telemetry?
2. Should we support only `executePendingShell` first, then add cancel path, or both together?
3. Should validation of `executionId` happen in agent layer or delegated to workspace tool layer with typed error mapping?
4. Do we keep temporary snake_case arg fallback in this gate, or require camelCase strictly now?

---

## H. Suggested implementation order

1. Add narrow Idle-continuation validator and state transition gate
2. Wire explicit error categories for failed validation
3. Add unit + integration tests
4. Add telemetry log for accepted Idle continuation path
5. Verify no regressions in cancel semantics

---

## Decision checkpoint

If agreed, implementation will follow this rule:

- **Default:** Idle messages are rejected (legacy safety preserved)
- **Exception:** Idle interactive continuation is accepted only after strict validation and atomic Busy transition

This keeps safety guarantees while fixing the real interactive UX bug.
