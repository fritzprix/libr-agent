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
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Default concurrency limits applied when settings are not yet loaded.
pub const DEFAULT_MAX_ACTIVE_AGENTS: u32 = 4;
pub const DEFAULT_MAX_SUSPENDED_AGENTS: u32 = 8;
pub const DEFAULT_MAX_ACTIVE_PROCESSES: u32 = 10;
pub const DEFAULT_MAX_SUSPENDED_PROCESSES: u32 = 20;

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
    pub async fn acquire_active_agent(&self) -> Result<(), String> {
        self.active_agent
            .acquire()
            .await
            .map_err(|_| "ConcurrencyGate: active agent semaphore closed".to_string())?
            .forget(); // Permit is released manually via release_active_agent().
        Ok(())
    }

    /// Release an active agent slot.
    /// Called when the workflow loop finishes (Idle / error).
    pub fn release_active_agent(&self) {
        self.active_agent.add_permits(1);
    }

    /// Active → Suspended transition (entry to `awaitAgent`).
    ///
    /// Acquires a suspended slot **before** releasing the active slot to prevent
    /// the TOCTOU window where both slots are briefly unoccupied.
    pub async fn suspend_agent(&self) -> Result<(), String> {
        self.suspended_agent
            .acquire()
            .await
            .map_err(|_| "ConcurrencyGate: suspended agent semaphore closed".to_string())?
            .forget();
        self.active_agent.add_permits(1);
        Ok(())
    }

    /// Suspended → Active transition (exit from `awaitAgent`).
    ///
    /// Re-acquires an active slot before releasing the suspended slot so the
    /// caller is guaranteed an active slot before considering itself resumed.
    pub async fn resume_agent(&self) -> Result<(), String> {
        self.active_agent
            .acquire()
            .await
            .map_err(|_| "ConcurrencyGate: active agent semaphore closed on resume".to_string())?
            .forget();
        self.suspended_agent.add_permits(1);
        Ok(())
    }

    // ── Process slots ────────────────────────────────────────────────────────

    /// Acquire an active process slot. Blocks until one is available.
    /// Called when a shell / code tool starts a new process.
    pub async fn acquire_active_process(&self) -> Result<(), String> {
        self.active_process
            .acquire()
            .await
            .map_err(|_| "ConcurrencyGate: active process semaphore closed".to_string())?
            .forget();
        Ok(())
    }

    /// Release an active process slot.
    pub fn release_active_process(&self) {
        self.active_process.add_permits(1);
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
        g.acquire_active_agent().await.unwrap();
        g.acquire_active_agent().await.unwrap();

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
        g.acquire_active_agent().await.unwrap();
        g.acquire_active_agent().await.unwrap();
        // All active slots taken — a new acquire would block.

        // One agent suspends (Active → Suspended), freeing an active slot.
        g.suspend_agent().await.unwrap();

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
        for _ in 0..4 {
            g.acquire_active_agent().await.unwrap();
        }

        // Suspend one: frees one active slot and uses one suspended slot.
        g.suspend_agent().await.unwrap();

        // Acquire the freed active slot (simulate a child starting).
        g.acquire_active_agent().await.unwrap();
        // active pool is full again.

        // Release one active slot so resume() has room.
        g.release_active_agent();
        // Resume: re-acquires active, releases suspended.
        g.resume_agent().await.unwrap();

        // Fully release all active slots and verify all 4 are available.
        for _ in 0..4 {
            g.release_active_agent();
        }
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
        g.acquire_active_process().await.unwrap();
        g.acquire_active_process().await.unwrap();

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
        g.release_active_process();
        let after_release = timeout(Duration::from_millis(20), g.acquire_active_process()).await;
        assert!(
            after_release.is_ok(),
            "Released process slot should be acquirable"
        );
    }
}
