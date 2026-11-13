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

/// Wait until something can answer LLM requests.
///
/// With the Rust executor on and no WebView, that is this process itself, so
/// there is nothing to wait for. With a WebView the wait stays, whatever the
/// executor setting: a provider the executor does not handle still goes to the
/// WebView, which must have announced itself or the request would be lost.
pub async fn wait_until_llm_executor_ready(app_handle: &tauri::AppHandle) {
    if crate::agent::llm::rust_executor::runs_without_webview(app_handle) {
        return;
    }
    wait_until_ready().await;
}
