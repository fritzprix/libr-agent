//! Deferred exit of the GUI process that hosts the shared backend (#2758).
//!
//! Closing the host's main window must not cut off other frontends attached
//! to its backend. While attachments are live, the close hides the window and
//! the process keeps serving; it exits once the attachment count reaches zero.
//! The exit does not wait for running turns: their sessions stay `busy` and
//! the next backend start recovers them to `paused`.

use crate::server::frontend_attachments::{self, HEARTBEAT_INTERVAL};
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, Window, WindowEvent};

static EXIT_WATCH_STARTED: AtomicBool = AtomicBool::new(false);

/// Whether closing the host window must hide it instead of ending the process.
pub fn should_defer_exit(hosts_backend: bool, live_attachments: usize) -> bool {
    hosts_backend && live_attachments > 0
}

/// Window-event hook: defer the main window's close while other frontends
/// are attached to this process's backend.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    if window.label() != "main" {
        return;
    }
    let hosts_backend = crate::state::get_shared_backend_client().is_none();
    let live = frontend_attachments::live_count();
    if !should_defer_exit(hosts_backend, live) {
        return;
    }
    api.prevent_close();
    if let Err(err) = window.hide() {
        warn!("Failed to hide main window on deferred exit: {err}");
    }
    info!("Main window closed with {live} attached frontend(s); backend keeps running");
    exit_when_detached(window.app_handle().clone());
}

fn exit_when_detached(app: AppHandle) {
    if EXIT_WATCH_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;
            if frontend_attachments::live_count() == 0 {
                info!("Last attached frontend is gone; shared backend exits");
                app.exit(0);
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::should_defer_exit;

    #[test]
    fn host_defers_exit_only_while_frontends_are_attached() {
        assert!(should_defer_exit(true, 1));
        assert!(should_defer_exit(true, 3));
        assert!(!should_defer_exit(true, 0));
    }

    #[test]
    fn attached_frontend_never_defers_its_own_exit() {
        assert!(!should_defer_exit(false, 2));
    }
}
