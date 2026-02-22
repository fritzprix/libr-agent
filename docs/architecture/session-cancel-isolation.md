# Session Cancel Isolation (SP6)

## Problem

When a parent agent session calls `awaitAgent` or `pollProcess`, it enters
`wait_until_session_terminal` and sleeps until either the child completes or a
30-second heartbeat fires. If the user cancels the parent session during this
wait, `cancel_workflow` sets `cancel_pending = true` and returns immediately
from the parent's perspective — but the tool call is still sleeping inside the
waiter loop, holding the parent's concurrency slot and delaying the actual stop
for up to 30 seconds.

## Solution: Dual-Notifier `tokio::select!`

### Key Insight

`cancel_workflow` already calls `SessionBus::notify_status_change` for the
child session when the child's status changes. SP6 extends this: when the
*parent* is cancelled, `cancel_workflow` also calls
`notify_status_change(parent_session_id)`. The waiter subscribes to **both**
the child's and the caller's bus entries, so whichever fires first wins.

### State Infrastructure (`state.rs`)

```rust
// Global shared Arc to the active sessions map (same Arc as AgentSessionManager).
// Allows builtin MCP handler code to access cancel tokens without Tauri managed-state.
static ACTIVE_SESSIONS: OnceLock<Arc<TokioRwLock<HashMap<String, AgentSession>>>> = OnceLock::new();

pub async fn get_session_cancel_pending(session_id: &str) -> Option<Arc<AtomicBool>> {
    let sessions = get_active_sessions().read().await;
    sessions.get(session_id).map(|s| s.cancel_pending.clone())
}
```

The read-lock is held only for the duration of the `clone()` — the returned
`Arc<AtomicBool>` can be polled lock-free in hot async loops.

### Wakeup Chain (`workflow.rs` → `handlers.rs`)

```
User clicks "Stop"
  → cancel_workflow(parent_id)
      → cancel_pending[parent_id] = true
      → cancellation_token.cancel()           // stops the LLM loop
      → SessionBus::notify_status_change(parent_id)  // ← SP6: wake the waiter
```

```rust
// handlers.rs — wait_until_session_terminal (simplified)
async fn wait_until_session_terminal(
    session_id: &str,
    timeout_seconds: u64,
    caller_session_id: Option<&str>,  // SP6: parent's session ID
) -> Result<(Value, u64), String> {
    let child_notifier  = bus.get_or_create(session_id);
    let caller_notifier = caller_session_id.map(|id| bus.get_or_create(id));
    let caller_cancel_pending = /* Arc<AtomicBool> cloned from ACTIVE_SESSIONS */;

    loop {
        // Fast path: check flag before hitting HTTP endpoint.
        if caller_cancel_pending.load(Relaxed) {
            return Err("awaitAgent interrupted: calling session was cancelled");
        }

        // ... HTTP status check ...

        tokio::select! {
            _ = child_notifier.notified()  => {}  // child status changed
            _ = caller_notify_branch       => {}  // parent cancelled  ← SP6
            _ = sleep(heartbeat_or_deadline) => {}
        }
    }
}
```

### Concurrency Slot Safety

The function is always called inside a `gate.suspend_agent()` / `gate.resume_agent()`
pair. When the function returns `Err` (due to parent cancel), the caller
immediately calls `gate.resume_agent()` — so the parent's active concurrency
slot is always released, even on the fast-cancel path.

## Data Flow Diagram

```
Parent session                    wait_until_session_terminal
─────────────────                 ──────────────────────────────────────
cancel_workflow()
  cancel_pending = true
  notify_status_change(parent) ──→ caller_notifier.notified() fires
                                    tokio::select! wakes
                                    check cancel_pending → true
                                    return Err("interrupted")
                                  ↓
                                gate.resume_agent()   ← slot released
```

## Regression Tests

Location: `src-tauri/src/agent/session_bus.rs` — `#[cfg(test)] mod tests`

| Test | Assertion |
|------|-----------|
| `test_sp6_caller_notify_wakes_dual_waiter` | Firing `notify_status_change(parent)` wakes the dual-notifier `select!` via the caller branch |
| `test_sp6_child_notify_still_wakes_dual_waiter` | Normal child-completion path still works with dual notifiers registered |
| `test_sp6_cancel_pending_flag_short_circuits_loop` | `AtomicBool = true` causes immediate `Err` return at loop entry |
| `test_sp6_quiet_caller_does_not_spuriously_wake` | No spurious wakeup when neither notifier fires; hits heartbeat/timeout branch |

## Related Files

| File | Change |
|------|--------|
| `src-tauri/src/state.rs` | `ACTIVE_SESSIONS` global, `init_active_sessions`, `get_session_cancel_pending` |
| `src-tauri/src/lifecycle/app_setup.rs` | `init_active_sessions(manager.active_sessions_arc())` |
| `src-tauri/src/agent/workflow.rs` | `notify_status_change(session_id)` in deferred cancel path |
| `src-tauri/src/mcp/builtin/session_api/handlers.rs` | dual-notifier `wait_until_session_terminal` |
| `src-tauri/src/agent/session_bus.rs` | SP6 regression tests |
