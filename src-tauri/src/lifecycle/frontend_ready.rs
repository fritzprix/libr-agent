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

/// Wait until the frontend is ready
pub async fn wait_until_ready() {
    if IS_READY.load(Ordering::SeqCst) {
        return;
    }
    READY_NOTIFY.notified().await;
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

/// Wait until something can answer LLM requests.
///
/// Rust-routed sessions need no frontend: when this process answers LLM calls
/// without a WebView (#2760 default routing), return immediately. When a
/// WebView is present, keep the `frontend-ready` wait — unsupported providers
/// and ignored Base URLs still go there, and a request would be lost if the
/// frontend had not announced itself.
pub async fn wait_until_llm_executor_ready(app_handle: &tauri::AppHandle) {
    if crate::agent::llm::rust_executor::runs_without_webview(app_handle) {
        return;
    }
    wait_until_ready().await;
}
