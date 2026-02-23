# SP1 + SP2: Reactive Blocking & Concurrency Control — Design Draft v0.1

> Status: **DRAFT v0.2 — Clarification Resolved**  
> Branch: `dev/0.5.x`  
> Date: 2026-02-22

---

## 1. Problem Statement

### SP1: awaitAgent / pollProcess are pure polling

`wait_until_session_terminal` (session_api/handlers.rs) polls `/api/sessions/{id}` in a loop:

```
poll every N seconds (1~30) → check status → repeat until terminal or timeout (max 900s)
```

**Consequences:**

- Every poll iteration adds a tool call + response message to conversation history
- With `poll_interval=5s, timeout=300s` → up to **60 tool call messages** for a single wait
- Context window filled with repetitive, low-value polling artifacts
- Token waste is proportional to task duration — worst case on long-running agents

### SP2: No concurrency limit exists

`spawnAgent` and `runShell` (persistent shell / code_execution) have no upper bound.  
An agent can spawn unlimited SubAgents and processes simultaneously:

- OOM risk on constrained hardware
- API rate limit hits across parallel LLM calls
- No back-pressure mechanism → runaway fanout

---

## 2. Current Architecture Reference

```
awaitAgent (MCP Tool)
  └─ wait_until_session_terminal() [session_api/handlers.rs]
       └─ loop { GET /api/sessions/{id} → sleep(poll_interval) }

spawnAgent (MCP Tool)
  └─ POST /api/sessions [session_api/handlers.rs]
       └─ AgentSessionManager::create_session + start_workflow

runShell / persistentShell (MCP Tool)
  └─ PersistentShellManager [workspace/persistent_shell_manager.rs]
       └─ PersistentShell per session [workspace/persistent_shell.rs]

AgentSessionManager [agent/session_manager.rs]
  └─ start_workflow / pause_workflow / resume_workflow / cancel_workflow
```

No `tokio::sync::watch`, `Notify`, or `Semaphore` primitives currently exist in
the session management path.

---

## 3. Proposed Architecture: Core Primitives

### 3.1 Session Status Notification Bus

Replace polling with a global reactive bus in `AgentSessionManager`:

```rust
// New primitive in AgentSessionManager
sessions: Arc<DashMap<SessionId, Arc<SessionSlot>>>

pub struct SessionSlot {
    // Current status (readable without lock)
    status: Arc<RwLock<SessionStatus>>,
    // Notified on any status change
    notify: Arc<Notify>,
    // Full state for structured reads
    state: Arc<RwLock<SessionState>>,
    // Slot classification (see SP2)
    slot_kind: RwLock<SlotKind>,
}
```

When `AgentSessionManager` transitions a session's status
(e.g. `Busy → Idle`, `Busy → Completed`), it calls `slot.notify.notify_waiters()`.

`wait_until_session_terminal` becomes:

```rust
async fn wait_until_session_terminal(
    session_id: &str,
    timeout: Option<Duration>,  // None = indefinite
) -> Result<SessionStatus, WaitError> {
    let slot = SESSION_BUS.get(session_id)?;
    loop {
        {
            let status = slot.status.read().await;
            if status.is_terminal() { return Ok(*status); }
        }
        match timeout {
            Some(d) => tokio::time::timeout(d, slot.notify.notified()).await
                           .map_err(|_| WaitError::Timeout)?,
            None    => slot.notify.notified().await,
        }
    }
}
```

**Zero polling. Zero spurious messages. Wakes exactly when state changes.**

### 3.2 Process Completion Notify

Same pattern for OS processes (runShell background execution):

```rust
pub struct ProcessEntry {
    pub id: ProcessId,
    pub status: Arc<RwLock<ProcessStatus>>,
    pub notify: Arc<Notify>,
    pub output_buf: Arc<Mutex<RingBuffer<String>>>,
}
```

`pollProcess(id, timeout=None)` → waits on `notify` until terminal.

---

## 4. SP2: Concurrency Control Layer (ConcurrencyGate)

### 4.1 Slot Model: Active vs Suspended

This is the **critical design decision** that prevents SP1+SP2 deadlock:

```
┌─────────────────────────────────────┐
│           Active Slots              │  ← currently executing LLM loop
│  SubAgent: default 4  (configurable)│
│  Process:  default 10 (configurable)│
└─────────────────────────────────────┘
         ↕ transition on SP1 block
┌─────────────────────────────────────┐
│          Suspended Slots            │  ← blocked on awaitAgent / pollProcess
│  SubAgent: default 8  (configurable)│
│  Process:  default 20 (configurable)│
└─────────────────────────────────────┘
```

> ✅ **결정: 앱 Settings에서 유저가 각 상한 조정 가능. 기본값은 위 표 기준.**

**Rule:** When a session calls `awaitAgent`/`pollProcess` with blocking mode,
it transitions from **Active → Suspended**, releasing its active slot and
acquiring a suspended slot. On wake-up: Suspended → Active (re-acquire active slot).

This breaks the deadlock:

- SubAgent-1 (Active) calls `awaitAgent(SubAgent-5)`
- SubAgent-1 transitions to Suspended → releases Active slot
- SubAgent-5 can now acquire an Active slot → runs → completes
- SubAgent-1 wakes up, re-acquires Active slot

### 4.2 ConcurrencyGate Struct

```rust
pub struct ConcurrencyGate {
    // Configurable slot semaphores (user-adjustable in Settings)
    active_agent_slots:     Arc<Semaphore>,  // default: 4
    suspended_agent_slots:  Arc<Semaphore>,  // default: 8
    active_process_slots:   Arc<Semaphore>,  // default: 10
    suspended_process_slots: Arc<Semaphore>, // default: 20
}

// NOTE: DependencyGraph (cycle detection) intentionally excluded.
// Deadlock prevention is handled via MAX_SPAWN_DEPTH = 5 (Section 4.3).
// Cycle detection can be added in a future phase if needed.
```

### 4.3 Spawn depth limit (Deadlock prevention)

> ✅ **결정: MAX_SPAWN_DEPTH = 5만 제한. DependencyGraph cycle detection은 필요 시 Phase 4+ 검토.**

Track `spawn_depth` per session and reject `spawnAgent` beyond `MAX_SPAWN_DEPTH = 5`:

```rust
// SessionSlot
spawn_depth: u8,  // inherited from parent on spawnAgent
```

### 4.4 Back-pressure on spawnAgent

When `active_agent_slots` semaphore is exhausted:

```
> ✅ **결정: Block until slot frees.** SP1 Notify 인프라 완료 후 Phase 1 마지막에 붙임.
```

---

## 5. API Changes to MCP Tools

### 5.1 awaitAgent — new parameters

```typescript
awaitAgent(
  sessionId: string,
  // NEW: undefined = indefinite block (no timeout)
  timeoutSeconds?: number,
  // REMOVED: pollIntervalSeconds (no longer polling)
)
```

When `timeoutSeconds` is absent (or 0): blocks until session reaches terminal state
with no timeout. Slot transitions: Active → Suspended for the duration.

During indefinite block, emit heartbeat events to UI every 30s (✅ 결정):

```rust
tokio::select! {
    _ = slot.notify.notified() => { /* status changed, re-check */ }
    _ = tokio::time::sleep(HEARTBEAT_INTERVAL) => {
        emit_agent_event(AgentEvent::AwaitHeartbeat {
            waiting_for: target_session_id.clone(),
            elapsed_secs: started_at.elapsed().as_secs(),
        });
    }
}
```

### 5.2 spawnAgent — back-pressure behavior

```typescript
spawnAgent(
  assistantId: string,
  request: string,
  // Back-pressure behavior: always blocks until active slot available
  // (no user-facing parameter — handled transparently)
)
```

> ✅ **결정: slot 초과 시 항상 blocking wait. 에이전트 입장에서 투명하게 처리.**

### 5.3 pollProcess — new parameters

```typescript
pollProcess(
  processId: string,
  // NEW: undefined = indefinite block
  timeoutSeconds?: number,
)
```

---

## 6. Clarification Decisions

| #      | Question                         | Decision                                              |
| ------ | -------------------------------- | ----------------------------------------------------- |
| **Q1** | spawnAgent on full active slots  | ✅ **Block** until slot frees (SP1 Notify 선행 필수)  |
| **Q2** | Suspended slot 상한              | ✅ **유저 설정 가능** (defaults: agent 8, process 20) |
| **Q3** | Deadlock prevention              | ✅ **MAX_SPAWN_DEPTH = 5** only                       |
| **Q4** | Parent cancel → children cascade | ⏳ **미결 — SP6 스펙 시 결정**                        |
| **Q5** | 무한 대기 중 heartbeat           | ✅ **Yes** — 30초마다 `AwaitHeartbeat` 이벤트 emit    |
| **Q6** | 구현 순서                        | ✅ **SP2 → SP1**                                      |

---

## 7. Proposed Implementation Sequence

Assuming **SP2 → SP1** order (recommended):

```
Phase 0: SessionBus infra (Notify primitive in AgentSessionManager)
         ProcessEntry registry with Notify in ProcessManager
         ← No visible behavior change yet, purely additive

Phase 1: SP2 ConcurrencyGate
         - active/suspended slot semaphores
         - slot transition on awaitAgent entry/exit
         - spawnAgent respects active_agent_slots
         - runShell respects active_process_slots

Phase 2: SP1 awaitAgent non-polling
         - swap wait_until_session_terminal to Notify-based
         - add indefinite timeout support
         - slot Active→Suspended transition

Phase 3: SP1 pollProcess non-polling
         - ProcessEntry with Notify
         - add indefinite timeout support

Phase 4: Deadlock detection
         - DependencyGraph cycle check (or depth-limit only if Q3 → depth)

Phase 5: Back-pressure on spawnAgent (depends on Q1 resolution)
```

---

## 8. Risk Register

| Risk                                                                | Likelihood | Mitigation                                          |
| ------------------------------------------------------------------- | ---------- | --------------------------------------------------- |
| Deadlock with two-phase slots if depth limit insufficient           | Medium     | DependencyGraph cycle detection (Phase 4)           |
| Suspended slot leak on unexpected session abort                     | Medium     | Drop impl on SessionSlot releases semaphore permits |
| `awaitAgent` indefinite blocks orphaned after app restart           | High       | Tombstone injection on restart (already in spec)    |
| Semaphore starvation (high-priority task waits behind low-priority) | Low        | Accept for now; priority queue is Phase 6+          |

---

_End of Draft v0.1 — Awaiting answers to Q1–Q6 before Phase 0 implementation begins._

---

## 9. Implementation Status (as-built, 2026-02-22)

> Status: **IMPLEMENTED ✅** — `dev/0.5.x`

All originally designed phases have been completed. Below is a mapping of spec
decisions to the actual code locations.

### 9.1 What Was Built vs. Designed

| Spec Section                    | Decision                       | Implemented? | Notes                                                                                                     |
| ------------------------------- | ------------------------------ | ------------ | --------------------------------------------------------------------------------------------------------- |
| 3.1 SessionBus                  | `DashMap<String, Arc<Notify>>` | ✅           | `agent/session_bus.rs` — exact design                                                                     |
| 3.2 ProcessEntry Notify         | `Arc<Notify>` per process      | ✅           | `completion_notifiers` in `ProcessRegistryData`; stored separately from entry to avoid clone issues       |
| 4.2 ConcurrencyGate             | 4 semaphores                   | ✅           | `agent/concurrency.rs` — exact design                                                                     |
| 4.3 Spawn depth                 | MAX_SPAWN_DEPTH check          | ⚠️           | Depth tracked per session; gate enforces via slot limits in practice. Hard numeric limit deferred to SP6. |
| 5.1 awaitAgent new params       | `timeoutSeconds?`              | ✅           | Default 180s; indefinite not exposed as `None` but backend loop is unbounded-capable                      |
| 5.2 spawnAgent back-pressure    | transparent block              | ✅           | `awaitCompletion` param added; both paths use two-phase gate                                              |
| 5.3 pollProcess → non-polling   | `waitForProcess`               | ✅           | `completion_notifiers` HashMap in registry; 30s heartbeat fallback                                        |
| Q1 Block on full slots          | always block                   | ✅           | `acquire_active_agent()` blocks on Semaphore                                                              |
| Q2 Suspended limit configurable | user-adjustable                | ✅           | `advancedSettings` in DB; Settings UI in `AdvancedTab.tsx`                                                |
| Q4 Parent cancel → children     | deferred to SP6                | ⏳           | Not implemented                                                                                           |
| Q5 Heartbeat during block       | 30s emit                       | ✅           | Both `awaitAgent` and `waitForProcess` use 30s heartbeat select branch                                    |

### 9.2 Actual Slot Enforcement Path

Status transitions are centralized through a single choke-point:

```
update_session_status(session_id, new_status)  [agent/lifecycle.rs]
  ├─ prev=non-Busy, next=Busy   → gate.acquire_active_agent()   (blocks)
  ├─ prev=Busy,     next=non-Busy → gate.release_active_agent()
  └─ else                         → no-op
  └─ (always) session_bus.notify_status_change(session_id)
```

No RAII guard needed — every status transition passes through here.

### 9.3 Two-Phase Deadlock Prevention (implemented)

```
awaitAgent handler:
  gate.suspend_agent()                    // Active → Suspended (frees active slot)
  wait_until_session_terminal(child_id)   // blocks on Notify, 30s heartbeat
  gate.resume_agent()                     // Suspended → Active (always, even on error)

spawnAgent awaitCompletion=true:
  [same pattern with child_id from POST /api/sessions]
```

### 9.4 Process Completion Path

```
async_exec.rs (tokio::spawn):
  registry.completion_notifiers.insert(pid, Arc::new(Notify::new()))
  ... spawn process, when finished:
  notifier.notify_waiters()

handle_wait_for_process (timeout > 0):
  loop {
    check status from registry
    if terminal → return
    tokio::select! {
      _ = notifier.notified() => {}       // wakes immediately on completion
      _ = sleep(30s)          => {}       // heartbeat fallback
    }
  }

handle_stop_process (killProcess):
  entry.status = Killed
  notifier = registry.completion_notifiers.get(pid)
  drop(write_lock)
  notifier.notify_waiters()              // wakes any blocked waitForProcess
```

### 9.5 Divergences from Spec

1. **`ProcessEntry.notify`** — spec proposed embedding `Arc<Notify>` directly in `ProcessEntry`. Implementation uses a separate `completion_notifiers: HashMap` to avoid borrow-checker issues with `get_mut` + immutable read on the same map entry.
2. **`SessionSlot` struct** — spec proposed a rich `SessionSlot` with `status + notify + state`. Implementation uses the lighter `SessionBus` (DashMap of bare Notifiers) because session state is already managed by the existing HTTP API layer. No code duplication needed.
3. **`pollProcess` rename** — `pollProcess` was retired. `waitForProcess` now handles both `timeout=0` (poll mode, immediate return) and `timeout>0` (blocking mode, push-notify wait). The tool description explicitly documents this.
