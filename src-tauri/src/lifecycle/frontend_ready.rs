use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

static IS_READY: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
static READY_NOTIFY: Lazy<Notify> = Lazy::new(Notify::new);

/// Mark the frontend as ready
pub fn mark_as_ready() {
    IS_READY.store(true, Ordering::SeqCst);
    READY_NOTIFY.notify_waiters();
}

/// Wait until the frontend is ready
pub async fn wait_until_ready() {
    if IS_READY.load(Ordering::SeqCst) {
        return;
    }
    READY_NOTIFY.notified().await;
}
