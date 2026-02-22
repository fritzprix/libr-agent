# Push-Notification Concurrency: Implementation Reference

> Branch: `dev/0.5.x` | Date: 2026-02-22  
> Implements: SP1 (reactive blocking) + SP2 (concurrency gate)  
> Spec: [`docs/specs/sp1-sp2-concurrency-design.md`](../specs/sp1-sp2-concurrency-design.md)

---

## Overview

Two orthogonal systems work together to eliminate polling and bound concurrency:

| System                        | What it solves                                | Core primitive                        |
| ----------------------------- | --------------------------------------------- | ------------------------------------- |
| **SessionBus**                | `awaitAgent` was polling every N seconds      | `tokio::sync::Notify` per session     |
| **ProcessRegistry notifiers** | `waitForProcess` was busy-polling every 100ms | `tokio::sync::Notify` per process     |
| **ConcurrencyGate**           | No upper bound on parallel agents / processes | `tokio::sync::Semaphore` (4 variants) |

---

## 1. SessionBus

**File:** `src-tauri/src/agent/session_bus.rs`  
**Singleton:** `state::SESSION_BUS: OnceLock<Arc<SessionBus>>`

```
┌───────────────────────────────────────────────────────────┐
│                        SessionBus                         │
│   DashMap<session_id: String, Arc<Notify>>                │
│                                                           │
│   notify_status_change(id)  →  notifier.notify_waiters()  │
│   get_or_create(id)         →  Arc<Notify>  (shared ref)  │
└───────────────────────────────────────────────────────────┘
```

**Fire side** (`agent/lifecycle.rs::update_session_status`):

```rust
// Every status transition passes through here — no RAII needed
crate::state::get_session_bus().notify_status_change(session_id);
```

**Wait side** (`session_api/handlers.rs::wait_until_session_terminal`):

```rust
let notifier = crate::state::get_session_bus().get_or_create(session_id);
loop {
    // check HTTP status (fast path: already terminal)
    tokio::select! {
        _ = notifier.notified() => {}        // wakes exactly when status changes
        _ = sleep(HEARTBEAT_30S)  => {}      // prevents orphaned wait if notify missed
    }
}
```

**Key property:** `get_or_create` never cleans up entries. Session IDs are bounded
by total session count (small). A late waiter registering after the session
completes will check status first (fast path returns immediately) — no missed event.

---

## 2. Process Completion Notifiers

**File:** `src-tauri/src/mcp/builtin/workspace/terminal_manager.rs`  
**Location in registry:**

```rust
pub struct ProcessRegistryData {
    pub entries:               HashMap<String, ProcessEntry>,
    pub cancellation_tokens:   HashMap<String, CancellationToken>,
    pub streaming_handles:     HashMap<String, Arc<StreamingHandle>>,
    pub completion_notifiers:  HashMap<String, Arc<Notify>>,  // ← added
}
```

**Why separate from `ProcessEntry`:** `handle_wait_for_process` calls
`entries.get_mut(id)` to update poll-tracking stats, then needs to read
`completion_notifiers.get(id)`. With `Notify` embedded in `ProcessEntry`,
this would be a borrow conflict (`&mut` + `&` on the same map). Keeping them
in a parallel HashMap avoids the issue cleanly.

### Lifecycle

| Event                          | Code location                               | Action                                                      |
| ------------------------------ | ------------------------------------------- | ----------------------------------------------------------- |
| Process registered             | `async_exec.rs`                             | `completion_notifiers.insert(pid, Arc::new(Notify::new()))` |
| Process completes (spawn task) | `async_exec.rs`                             | `notifier.notify_waiters()`                                 |
| Process killed                 | `handlers/terminal.rs::handle_stop_process` | `notifier.notify_waiters()` after `drop(write_lock)`        |
| Old process cleaned up         | `mod.rs::cleanup_old_processes`             | `completion_notifiers.remove(id)`                           |
| Session ends                   | `mod.rs::on_session_end`                    | `completion_notifiers.remove(id)`                           |

**Fire order in `handle_stop_process`:**

```rust
// 1. Set status = Killed  (under write lock)
// 2. Clone Arc<Notify>    (still under write lock)
let notifier = registry.completion_notifiers.get(process_id).cloned();
// 3. Drop write lock BEFORE firing — prevents lock inversion with blocked waiter
drop(registry);
// 4. Fire
if let Some(n) = notifier { n.notify_waiters(); }
```

### Wait side (`handlers/terminal.rs::handle_wait_for_process`)

Blocking mode (`timeout_secs > 0`):

```rust
loop {
    // 1. Write-lock registry: update poll stats, clone status + notifier
    let (status, entry_data, guidance, notifier) = { ... registry.write() ... };
    // (write lock released here)

    // 2. Check terminal — return immediately if done
    if is_terminal(status) { return ...; }

    // 3. Timeout check
    if start.elapsed() >= timeout { return timeout_error; }

    // 4. Sleep efficiently — wake on notify OR 30s heartbeat
    match notifier {
        Some(n) => tokio::select! {
            _ = n.notified() => {}
            _ = sleep(30s)   => {}
        },
        None => sleep(500ms).await,   // defensive fallback
    }
}
```

Polling mode (`timeout_secs == 0`): single iteration, return immediately.

---

## 3. ConcurrencyGate

**File:** `src-tauri/src/agent/concurrency.rs`  
**Singleton:** `state::CONCURRENCY_GATE: OnceLock<Arc<ConcurrencyGate>>`  
**Initialized:** `lifecycle/app_setup.rs` — reads `advancedSettings` from DB

```
┌─────────────────────────────────────────────────────────────┐
│                     ConcurrencyGate                         │
│                                                             │
│  active_agent    Semaphore(4)   ── agents running LLM loop  │
│  suspended_agent Semaphore(8)   ── agents blocked on await  │
│  active_process  Semaphore(10)  ── shell processes running  │
│  suspended_process Semaphore(20)── processes blocked on wait│
└─────────────────────────────────────────────────────────────┘
```

### Default Limits (user-configurable in Settings → Advanced)

| Semaphore           | Default | Setting Key             |
| ------------------- | ------- | ----------------------- |
| `active_agent`      | 4       | `maxActiveAgents`       |
| `suspended_agent`   | 8       | `maxSuspendedAgents`    |
| `active_process`    | 10      | `maxActiveProcesses`    |
| `suspended_process` | 20      | `maxSuspendedProcesses` |

### Slot Enforcement — Single Choke-Point

All agent status transitions pass through `update_session_status` in `lifecycle.rs`:

```rust
match (is_prev_busy, is_next_busy) {
    (false, true)  => gate.acquire_active_agent().await?,  // blocks if full
    (true,  false) => gate.release_active_agent(),          // frees slot
    _              => {}                                     // no-op
}
```

No RAII guard required. No risk of double-release or missed release.

### Two-Phase Transition (Deadlock Prevention)

Without the two-phase model, if all 4 active slots are held by parents waiting
for children, and children cannot start (no active slots), the system deadlocks.

**Solution:** Parent transitions `Active → Suspended` before waiting:

```
Parent calls awaitAgent:
  1. gate.suspend_agent()          // acquire suspended slot, release active slot
                                   // ← child can now start (active slot freed)
  2. wait_until_session_terminal() // blocks on Notify
  3. gate.resume_agent()           // acquire active slot, release suspended slot
                                   // (always called — even on timeout/error)
```

`suspend_agent` intentionally acquires suspended **before** releasing active
to prevent a TOCTOU window where both slots are momentarily unoccupied.

### spawnAgent `awaitCompletion=true`

Same two-phase pattern applied inline:

```
POST /api/sessions           → child_id
gate.suspend_agent()
wait_until_session_terminal(child_id, timeout)
gate.resume_agent()
→ return combined spawn + completion result
```

Result format matches `awaitAgent` exactly: `{ session, status, pollCount, messages }`.

---

## 4. Data Flow Diagram

```
User / LLM Tool Call
        │
        ▼
spawnAgent ──────────────────────────────── awaitCompletion=false ──→ return immediately
        │
        │ awaitCompletion=true (or awaitAgent)
        ▼
gate.suspend_agent()  ← acquire suspended, release active
        │
        ▼
wait_until_session_terminal(child_id)
  ├─ check HTTP status (fast path)
  └─ tokio::select! { SessionBus.notified() | sleep(30s) }
              ▲
              │  fired by
              │
update_session_status(child_id, Idle/Error)   [lifecycle.rs]
  ├─ gate.release_active_agent()   (child's slot freed)
  └─ SESSION_BUS.notify_status_change(child_id)  ← wakes parent
        │
        ▼
gate.resume_agent()  ← acquire active, release suspended
        │
        ▼
return result to LLM
```

---

## 5. Process Gate Wire-Up

`ConcurrencyGate::acquire_active_process` / `release_active_process` are wired into
`async_exec.rs::execute_shell_async`.

**Acquire** — called after all early-return guards (session limit check, isolation
command creation) but before registry registration, so no registry cleanup is
needed if the gate is blocked or closed:

```rust
// async_exec.rs — before registry.entries.insert()
crate::state::get_concurrency_gate()
    .acquire_active_process()
    .await?;
```

**Release** — called at the very end of the `tokio::spawn` task body, after the
completion notifier fires:

```rust
// async_exec.rs — end of spawn task
crate::state::get_concurrency_gate().release_active_process();
```

The `&'static ConcurrencyGate` reference is obtained via `crate::state::get_concurrency_gate()`
(a `OnceLock`-backed global), which is move-safe across the async spawn boundary.

---

## 6. Test Coverage

See inline `#[cfg(test)]` modules in:

- `src-tauri/src/agent/concurrency.rs` — slot semantics, two-phase correctness
- `src-tauri/src/agent/session_bus.rs` — notify/wake, pre-notify no-op
- `src-tauri/src/mcp/builtin/workspace/terminal_manager.rs` — registry field, notifier wake

Run with:

```sh
cd src-tauri && cargo test sp1_sp2 -- --nocapture
```
