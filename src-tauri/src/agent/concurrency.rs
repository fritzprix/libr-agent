/// Global concurrency gate for controlling parallel execution limits.
///
/// Implements the two-phase slot model to prevent deadlock:
///  - Active slots  : sessions/processes actively running their LLM / shell loop.
///  - Suspended slots: sessions/processes blocked on `awaitAgent` / `pollProcess`.
///
/// When a parent calls `awaitAgent` it transitions Active → Suspended:
///   1. Acquires a suspended slot (blocks until one is free).
///   2. Releases its active slot (opens room for a child session to run).
///
/// When `awaitAgent` returns the inverse happens:
///   1. Re-acquires an active slot (blocks until one is free).
///   2. Releases the suspended slot.
///
/// This prevents the classic deadlock where every active slot is occupied by a
/// parent waiting on children that can never start because all active slots are
/// taken.
use crate::agent::state::AgentSession;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use tokio_util::sync::CancellationToken;

/// Default concurrency limits applied when settings are not yet loaded.
pub const DEFAULT_MAX_ACTIVE_AGENTS: u32 = 4;
pub const DEFAULT_MAX_SUSPENDED_AGENTS: u32 = 8;
pub const DEFAULT_MAX_ACTIVE_PROCESSES: u32 = 10;
pub const DEFAULT_MAX_SUSPENDED_PROCESSES: u32 = 20;

#[derive(Debug)]
pub struct ConcurrencyGate {
    /// Sessions actively executing their LLM loop.
    active_agent: Arc<Semaphore>,
    /// Sessions blocked on `awaitAgent` waiting for a child to finish.
    suspended_agent: Arc<Semaphore>,
    /// Shell / code processes actively running.
    active_process: Arc<Semaphore>,
    /// Processes blocked on `pollProcess`.
    suspended_process: Arc<Semaphore>,
}

#[derive(Debug)]
pub struct ActiveAgentPermit {
    _permit: OwnedSemaphorePermit,
}

impl ActiveAgentPermit {
    fn new(permit: OwnedSemaphorePermit) -> Self {
        Self { _permit: permit }
    }
}

#[derive(Debug)]
pub struct SuspendedAgentGuard<'a> {
    gate: &'a ConcurrencyGate,
    _suspended_permit: OwnedSemaphorePermit,
}

impl<'a> SuspendedAgentGuard<'a> {
    fn new(gate: &'a ConcurrencyGate, suspended_permit: OwnedSemaphorePermit) -> Self {
        Self {
            gate,
            _suspended_permit: suspended_permit,
        }
    }

    pub async fn resume(self) -> Result<ActiveAgentPermit, String> {
        let Self {
            gate,
            _suspended_permit,
        } = self;
        let active_permit = gate.acquire_active_agent().await?;
        drop(_suspended_permit);
        Ok(active_permit)
    }

    pub async fn resume_cancellable(
        self,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> Result<ActiveAgentPermit, String> {
        let Self {
            gate,
            _suspended_permit,
        } = self;
        let active_permit = gate.acquire_active_agent_cancellable(cancel).await?;
        drop(_suspended_permit);
        Ok(active_permit)
    }

    /// Try to re-acquire an active slot without blocking. Releases the suspended slot either way.
    pub fn try_resume(self) -> Option<ActiveAgentPermit> {
        let Self {
            gate,
            _suspended_permit,
        } = self;
        let active_permit = gate.try_acquire_active_agent();
        drop(_suspended_permit);
        active_permit
    }
}

#[derive(Debug)]
pub struct ActiveProcessPermit {
    _permit: OwnedSemaphorePermit,
}

impl ActiveProcessPermit {
    fn new(permit: OwnedSemaphorePermit) -> Self {
        Self { _permit: permit }
    }
}

impl ConcurrencyGate {
    pub fn new(
        max_active_agents: u32,
        max_suspended_agents: u32,
        max_active_processes: u32,
        max_suspended_processes: u32,
    ) -> Self {
        Self {
            active_agent: Arc::new(Semaphore::new(max_active_agents as usize)),
            suspended_agent: Arc::new(Semaphore::new(max_suspended_agents as usize)),
            active_process: Arc::new(Semaphore::new(max_active_processes as usize)),
            suspended_process: Arc::new(Semaphore::new(max_suspended_processes as usize)),
        }
    }

    // ── Agent slots ─────────────────────────────────────────────────────────

    /// Acquire an active agent slot. Blocks until one is available.
    /// Called when `start_workflow` begins a new LLM execution loop.
    pub async fn acquire_active_agent(&self) -> Result<ActiveAgentPermit, String> {
        let permit = Arc::clone(&self.active_agent)
            .acquire_owned()
            .await
            .map_err(|_| "ConcurrencyGate: active agent semaphore closed".to_string())?;
        Ok(ActiveAgentPermit::new(permit))
    }

    /// Try to acquire an active agent slot without blocking.
    pub fn try_acquire_active_agent(&self) -> Option<ActiveAgentPermit> {
        Arc::clone(&self.active_agent)
            .try_acquire_owned()
            .ok()
            .map(ActiveAgentPermit::new)
    }

    /// Acquire an active agent slot, aborting immediately if `cancel` is triggered.
    pub async fn acquire_active_agent_cancellable(
        &self,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> Result<ActiveAgentPermit, String> {
        if cancel.is_cancelled() {
            return Err(
                "Session cancelled while waiting for an active concurrency slot".to_string(),
            );
        }
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                Err("Session cancelled while waiting for an active concurrency slot".to_string())
            }
            permit = self.acquire_active_agent() => permit,
        }
    }

    /// Active → Suspended transition (entry to `awaitAgent`).
    ///
    /// Acquires a suspended slot **before** releasing the active slot to prevent
    /// the TOCTOU window where both slots are briefly unoccupied.
    pub async fn suspend_agent<'a>(
        &'a self,
        active_permit: &mut Option<ActiveAgentPermit>,
    ) -> Result<SuspendedAgentGuard<'a>, String> {
        let suspended_permit = Arc::clone(&self.suspended_agent)
            .acquire_owned()
            .await
            .map_err(|_| "ConcurrencyGate: suspended agent semaphore closed".to_string())?;
        drop(active_permit.take());
        Ok(SuspendedAgentGuard::new(self, suspended_permit))
    }

    // ── Process slots ────────────────────────────────────────────────────────

    /// Acquire an active process slot. Blocks until one is available.
    /// Called when a shell / code tool starts a new process.
    pub async fn acquire_active_process(&self) -> Result<ActiveProcessPermit, String> {
        let permit = Arc::clone(&self.active_process)
            .acquire_owned()
            .await
            .map_err(|_| "ConcurrencyGate: active process semaphore closed".to_string())?;
        Ok(ActiveProcessPermit::new(permit))
    }

    /// Active → Suspended transition for a process (entry to `pollProcess` wait).
    pub async fn suspend_process(&self) -> Result<(), String> {
        self.suspended_process
            .acquire()
            .await
            .map_err(|_| "ConcurrencyGate: suspended process semaphore closed".to_string())?
            .forget();
        self.active_process.add_permits(1);
        Ok(())
    }

    /// Suspended → Active transition for a process (exit from `pollProcess` wait).
    pub async fn resume_process(&self) -> Result<(), String> {
        self.active_process
            .acquire()
            .await
            .map_err(|_| "ConcurrencyGate: active process semaphore closed on resume".to_string())?
            .forget();
        self.suspended_process.add_permits(1);
        Ok(())
    }
}

/// Look up a session's cancellation token. `None` when the session is not
/// active — callers must handle that explicitly instead of silently waiting on
/// a fresh token that can never fire (#3086 G1.4).
pub async fn session_cancel_token(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Option<CancellationToken> {
    active_sessions
        .read()
        .await
        .get(session_id)
        .map(|session| session.cancellation_token.clone())
}

/// Why a session-attached queued active-slot acquire did not yield a permit.
#[derive(Debug)]
pub enum QueuedPermitError {
    /// The session's own cancellation token fired while it was queued.
    Cancelled,
    /// The session was already gone: there is no token to cancel the wait with.
    MissingSession,
    /// The gate itself failed (closed semaphore).
    Gate(String),
}

/// Acquire the active slot for a queued session, aborting when the session is
/// cancelled while queued. One implementation shared by background start /
/// resume / injection and the LLM-turn permit guard, so the token lookup and
/// the cancelled arm cannot drift apart (#3086 G1.4).
///
/// `gate` is passed explicitly (callers use `crate::state::get_concurrency_gate()`)
/// so the queue/cancel semantics stay testable without global state.
pub async fn acquire_queued_permit(
    gate: &ConcurrencyGate,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<ActiveAgentPermit, QueuedPermitError> {
    let cancel = match session_cancel_token(active_sessions, session_id).await {
        Some(token) => token,
        None => return Err(QueuedPermitError::MissingSession),
    };
    match gate.acquire_active_agent_cancellable(&cancel).await {
        Ok(permit) => Ok(permit),
        Err(_) if cancel.is_cancelled() => Err(QueuedPermitError::Cancelled),
        Err(error) => Err(QueuedPermitError::Gate(error)),
    }
}

// ─── Regression Tests (SP1 + SP2) ────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    fn gate(active: u32, suspended: u32) -> ConcurrencyGate {
        ConcurrencyGate::new(active, suspended, 10, 20)
    }

    /// SP2: active slot limit is enforced — a (limit+1)-th acquire must block.
    #[tokio::test]
    async fn test_sp1_sp2_active_slot_limit_enforced() {
        let g = gate(2, 4);
        let _permit1 = g.acquire_active_agent().await.unwrap();
        let _permit2 = g.acquire_active_agent().await.unwrap();

        // Third acquire must not complete before a slot is freed.
        let blocked = timeout(Duration::from_millis(30), g.acquire_active_agent()).await;
        assert!(
            blocked.is_err(),
            "Third acquire should block when all 2 active slots are taken"
        );
    }

    /// SP2: suspending frees an active slot so a new agent can start.
    #[tokio::test]
    async fn test_sp1_sp2_suspend_frees_active_slot() {
        let g = Arc::new(gate(2, 4));
        let mut permit1 = Some(g.acquire_active_agent().await.unwrap());
        let _permit2 = g.acquire_active_agent().await.unwrap();
        // All active slots taken — a new acquire would block.

        // One agent suspends (Active → Suspended), freeing an active slot.
        let _suspended = g.suspend_agent(&mut permit1).await.unwrap();

        // Now a new agent should be able to acquire the freed active slot quickly.
        let g2 = Arc::clone(&g);
        let acquired = timeout(
            Duration::from_millis(50),
            tokio::spawn(async move { g2.acquire_active_agent().await.unwrap() }),
        )
        .await;
        assert!(
            acquired.is_ok(),
            "Should acquire active slot after one suspension"
        );
    }

    /// SP1+SP2: two-phase suspend/resume roundtrip leaves slot counts balanced.
    #[tokio::test]
    async fn test_sp1_sp2_suspend_resume_balances_slots() {
        let g = gate(4, 8);
        // Fill all 4 active slots.
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(g.acquire_active_agent().await.unwrap());
        }

        // Suspend one: frees one active slot and uses one suspended slot.
        let mut suspended_permit = Some(permits.pop().unwrap());
        let suspended = g.suspend_agent(&mut suspended_permit).await.unwrap();

        // Acquire the freed active slot (simulate a child starting).
        let child_permit = g.acquire_active_agent().await.unwrap();
        // active pool is full again.

        // Release one active slot so resume() has room.
        drop(child_permit);
        // Resume: re-acquires active, releases suspended.
        let resumed = suspended.resume().await.unwrap();
        permits.push(resumed);

        // Fully release all active slots and verify all 4 are available.
        drop(permits);
        for _ in 0..4 {
            let r = timeout(Duration::from_millis(10), g.acquire_active_agent()).await;
            assert!(
                r.is_ok(),
                "All 4 active slots should be available after full roundtrip"
            );
        }
    }

    /// SP2: process slot acquire/release is independent from agent slots.
    #[tokio::test]
    async fn test_sp1_sp2_process_slots_independent() {
        let g = ConcurrencyGate::new(4, 8, 2, 4);
        let _process1 = g.acquire_active_process().await.unwrap();
        let process2 = g.acquire_active_process().await.unwrap();

        // 3rd process acquire should block (limit = 2)
        let blocked = timeout(Duration::from_millis(30), g.acquire_active_process()).await;
        assert!(
            blocked.is_err(),
            "Process slot limit should be enforced independently"
        );

        // Agent slots are unaffected
        let agent_ok = timeout(Duration::from_millis(10), g.acquire_active_agent()).await;
        assert!(
            agent_ok.is_ok(),
            "Agent slots must be independent from process slots"
        );

        // Release a process slot and verify it becomes available
        drop(process2);
        let after_release = timeout(Duration::from_millis(20), g.acquire_active_process()).await;
        assert!(
            after_release.is_ok(),
            "Released process slot should be acquirable"
        );
    }

    #[tokio::test]
    async fn test_acquire_active_agent_cancellable_aborts_queued_waiter_immediately() {
        let g = Arc::new(gate(1, 2));
        let _held = g.acquire_active_agent().await.unwrap();

        let token = tokio_util::sync::CancellationToken::new();
        let token_clone = token.clone();
        let g_clone = Arc::clone(&g);

        let waiter =
            tokio::spawn(
                async move { g_clone.acquire_active_agent_cancellable(&token_clone).await },
            );

        tokio::task::yield_now().await;
        token.cancel();

        let res = waiter.await.unwrap();
        assert!(
            res.is_err(),
            "Cancelled queued waiter must abort without waiting for a slot"
        );

        // Dropping the held permit leaves the slot immediately available to a new waiter
        // (proving the cancelled waiter left the semaphore queue).
        drop(_held);
        let next = g.try_acquire_active_agent();
        assert!(
            next.is_some(),
            "Slot must be free after cancelled waiter leaves the queue"
        );
    }

    #[tokio::test]
    async fn test_resume_cancellable_aborts_and_releases_suspended_slot() {
        let g = Arc::new(gate(1, 1));
        let mut parent_permit = Some(g.acquire_active_agent().await.unwrap());
        let suspended = g.suspend_agent(&mut parent_permit).await.unwrap();

        // Fill the only active slot so resume cannot proceed.
        let _other = g.acquire_active_agent().await.unwrap();

        let token = tokio_util::sync::CancellationToken::new();
        let token_clone = token.clone();
        let resume_fut = suspended.resume_cancellable(&token_clone);
        token.cancel();

        let res = resume_fut.await;
        assert!(
            res.is_err(),
            "Cancelled resume must abort without waiting for an active slot"
        );

        // Suspended slot must have been released on abort.
        let mut another_active = Some(_other);
        let suspended_again = g.suspend_agent(&mut another_active).await;
        assert!(
            suspended_again.is_ok(),
            "Suspended slot must be released when resume_cancellable aborts"
        );
    }
}
