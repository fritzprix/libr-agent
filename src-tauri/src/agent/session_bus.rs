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
}
