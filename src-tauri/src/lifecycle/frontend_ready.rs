use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

static IS_READY: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static READY_NOTIFY: Lazy<Notify> = Lazy::new(Notify::new);

/// Mark the frontend as ready
pub fn mark_as_ready() {
    IS_READY.store(true, Ordering::SeqCst);
    READY_NOTIFY.notify_waiters();
}

/// Whether the frontend has already announced readiness.
pub fn is_ready() -> bool {
    IS_READY.load(Ordering::SeqCst)
}

/// Wait until the frontend is ready.
///
/// The notification is registered before the flag is read again: `notify_waiters`
/// drops a wakeup that nobody is waiting for, so a frontend that signals
/// readiness between the first check and the wait would otherwise hang the
/// caller until its timeout.
pub async fn wait_until_ready() {
    while !is_ready() {
        let notified = READY_NOTIFY.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if is_ready() {
            return;
        }
        notified.await;
    }
}

/// Blocking wait for frontend readiness (TUI front thread on Windows).
///
/// Returns `true` when ready, `false` on timeout.
pub fn wait_until_ready_blocking(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while !is_ready() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_wait_observes_the_readiness_flag() {
        // IS_READY is one-way global state, so both sides run in one test,
        // false before true. A sibling test that flipped the flag first
        // still leaves this one deterministic.
        if !is_ready() {
            assert!(!wait_until_ready_blocking(Duration::from_millis(1)));
        }
        mark_as_ready();
        assert!(is_ready());
        assert!(wait_until_ready_blocking(Duration::from_millis(1)));
    }
}
