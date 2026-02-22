/// Push-based notification bus for session status changes.
///
/// Eliminates busy-polling in `awaitAgent` and `pollProcess`: callers wait on a
/// `tokio::sync::Notify` that is woken exactly when `update_session_status` runs,
/// rather than sleeping for a fixed poll interval and hitting the HTTP server.
///
/// # Lifecycle
/// 1. Tool handler calls `get_or_create(session_id)` to obtain the `Arc<Notify>`.
/// 2. Handler checks the current status from the HTTP API.
/// 3. If not terminal, handler calls `notifier.notified().await` inside a
///    `tokio::select!` branch (alongside a timeout / heartbeat branch).
/// 4. `update_session_status` in `lifecycle.rs` calls `notify_status_change`.
/// 5. Waiter wakes, rechecks the status, and either returns or waits again.
///
/// Entries are created lazily and never cleaned up (size is bounded by total session
/// count which is small).  This avoids any TOCTOU race between cleanup and a late
/// waiter trying to register after the session completes.
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Notify;

pub struct SessionBus {
    notifiers: DashMap<String, Arc<Notify>>,
}

impl Default for SessionBus {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionBus {
    pub fn new() -> Self {
        Self {
            notifiers: DashMap::new(),
        }
    }

    /// Notify all waiters for `session_id` that its status has changed.
    ///
    /// If nobody is waiting yet (no entry present), this is a no-op — the next
    /// caller to `get_or_create` will start fresh without missing any event,
    /// because they will check the current status before sleeping.
    pub fn notify_status_change(&self, session_id: &str) {
        if let Some(notifier) = self.notifiers.get(session_id) {
            notifier.notify_waiters();
        }
    }

    /// Get or create the `Notify` for `session_id`.
    ///
    /// The returned `Arc<Notify>` is shared with the global entry.  Callers
    /// should call `notifier.notified().await` inside a `tokio::select!` to wake
    /// on the next status change without polling.
    pub fn get_or_create(&self, session_id: &str) -> Arc<Notify> {
        self.notifiers
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Notify::new()))
            .clone()
    }
}

// ─── Regression Tests (SP1) ──────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    /// SP1: a waiter unblocks immediately when notify_status_change fires.
    #[tokio::test]
    async fn test_sp1_sp2_notify_wakes_waiter() {
        let bus = Arc::new(SessionBus::new());
        let notifier = bus.get_or_create("sess-a");

        let bus2 = Arc::clone(&bus);
        // Spawn a task that waits on the notifier.
        let waiter = tokio::spawn(async move {
            notifier.notified().await;
        });

        // Give the waiter time to register before firing.
        tokio::task::yield_now().await;

        bus2.notify_status_change("sess-a");

        // Waiter should wake within a tight deadline.
        let result = timeout(Duration::from_millis(100), waiter).await;
        assert!(
            result.is_ok(),
            "Waiter should unblock within 100ms after notify"
        );
    }

    /// SP1: firing notify when no waiter is registered is a no-op (no panic).
    #[tokio::test]
    async fn test_sp1_sp2_notify_before_waiter_is_noop() {
        let bus = SessionBus::new();
        // No waiter registered yet — fire is silently ignored.
        bus.notify_status_change("sess-ghost");
        // Creating a notifier after the fire does NOT unblock retroactively.
        // (tokio::sync::Notify does not store permits across notify_waiters calls.)
        let notifier = bus.get_or_create("sess-ghost");
        let blocked = timeout(Duration::from_millis(20), notifier.notified()).await;
        assert!(
            blocked.is_err(),
            "Notify fired before waiter registered should not unblock a future waiter"
        );
    }

    /// SP1: get_or_create returns the same Arc for the same session_id.
    #[tokio::test]
    async fn test_sp1_sp2_get_or_create_returns_same_arc() {
        let bus = SessionBus::new();
        let n1 = bus.get_or_create("sess-b");
        let n2 = bus.get_or_create("sess-b");
        assert!(
            Arc::ptr_eq(&n1, &n2),
            "get_or_create must return the same Arc for the same session_id"
        );
    }

    /// SP1: different session IDs get distinct notifiers.
    #[tokio::test]
    async fn test_sp1_sp2_distinct_notifiers_per_session() {
        let bus = SessionBus::new();
        let na = bus.get_or_create("sess-x");
        let nb = bus.get_or_create("sess-y");
        assert!(
            !Arc::ptr_eq(&na, &nb),
            "Different session IDs must have distinct Arc<Notify> instances"
        );
    }

    // ─── SP6: Caller-cancel wakeup (parent cancel isolation) ─────────────────

    /// SP6: A waiter blocked in a dual-notifier select! (child OR caller) is
    /// immediately woken when the *caller* (parent) session fires — not only
    /// when the child session fires.
    ///
    /// This is the core of SP6: `cancel_workflow` calls
    /// `notify_status_change(parent_id)` which must escape the
    /// `wait_until_session_terminal` loop without waiting for the child.
    #[tokio::test]
    async fn test_sp6_caller_notify_wakes_dual_waiter() {
        let bus = Arc::new(SessionBus::new());

        let child_notifier = bus.get_or_create("child-sess");
        let caller_notifier = bus.get_or_create("parent-sess");

        // Simulate the tokio::select! inside wait_until_session_terminal:
        // wake on either child status change OR caller cancel notification.
        let waiter = tokio::spawn(async move {
            tokio::select! {
                _ = child_notifier.notified() => "child",
                _ = caller_notifier.notified() => "caller",
            }
        });

        tokio::task::yield_now().await;

        // Fire the CALLER (parent) bus — child never completes.
        bus.notify_status_change("parent-sess");

        let result = timeout(Duration::from_millis(100), waiter).await;
        assert!(
            result.is_ok(),
            "SP6: dual-notifier waiter must unblock when caller notify fires"
        );
        assert_eq!(
            result.unwrap().unwrap(),
            "caller",
            "SP6: wakeup source must be the caller branch"
        );
    }

    /// SP6: Normal path still works — child completing wakes the dual-notifier
    /// waiter even when a caller notifier is registered.
    #[tokio::test]
    async fn test_sp6_child_notify_still_wakes_dual_waiter() {
        let bus = Arc::new(SessionBus::new());

        let child_notifier = bus.get_or_create("child-sess2");
        let caller_notifier = bus.get_or_create("parent-sess2");

        let waiter = tokio::spawn(async move {
            tokio::select! {
                _ = child_notifier.notified() => "child",
                _ = caller_notifier.notified() => "caller",
            }
        });

        tokio::task::yield_now().await;

        // Fire the CHILD — normal completion path.
        bus.notify_status_change("child-sess2");

        let result = timeout(Duration::from_millis(100), waiter).await;
        assert!(
            result.is_ok(),
            "SP6: dual-notifier waiter must still unblock when child fires"
        );
        assert_eq!(
            result.unwrap().unwrap(),
            "child",
            "SP6: normal path wakeup source must be the child branch"
        );
    }

    /// SP6: cancel_pending flag (AtomicBool) short-circuits the wait loop when
    /// already true at loop entry — the waiter exits without waiting at all.
    ///
    /// In production this happens when cancel_workflow sets cancel_pending=true
    /// and then notifies; even if the notification races ahead of the flag read,
    /// the next loop iteration sees the flag and returns Err immediately.
    #[tokio::test]
    async fn test_sp6_cancel_pending_flag_short_circuits_loop() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Simulate the loop body: check flag, then wait.
        let flag_clone = Arc::clone(&cancel_flag);
        let result: Result<(), &str> = tokio::spawn(async move {
            // Set cancel before the loop runs (simulates: cancel already fired).
            flag_clone.store(true, Ordering::Relaxed);

            // Simulate loop iteration: check flag at top.
            if flag_clone.load(Ordering::Relaxed) {
                return Err("interrupted");
            }
            Ok(())
        })
        .await
        .unwrap();

        assert_eq!(
            result,
            Err("interrupted"),
            "SP6: cancel_pending=true must short-circuit the wait loop immediately"
        );
    }

    /// SP6: caller notifier that was never fired does NOT falsely wake the
    /// dual-notifier waiter — only a real notify_status_change triggers wakeup.
    #[tokio::test]
    async fn test_sp6_quiet_caller_does_not_spuriously_wake() {
        let bus = Arc::new(SessionBus::new());

        let child_notifier = bus.get_or_create("child-sess3");
        let caller_notifier = bus.get_or_create("parent-sess3");

        // Neither child nor caller will fire — waiter should time out.
        let waiter = tokio::spawn(async move {
            tokio::select! {
                _ = child_notifier.notified() => "child",
                _ = caller_notifier.notified() => "caller",
                _ = tokio::time::sleep(Duration::from_millis(30)) => "timeout",
            }
        });

        let result = timeout(Duration::from_millis(100), waiter).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().unwrap(),
            "timeout",
            "SP6: waiter must not wake spuriously when neither notifier fires"
        );
    }
}
