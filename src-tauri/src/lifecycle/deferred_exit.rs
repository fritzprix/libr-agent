//! Deferred exit of the GUI process that hosts the shared backend (#2758).
//!
//! Closing the host's main window must not cut off other frontends attached
//! to its backend. While attachments are live, every user-initiated quit is
//! deferred: closing the main window (`WindowEvent::CloseRequested`) and any
//! other quit route that reaches the run loop as `RunEvent::ExitRequested`
//! without an exit code (for example Cmd+Q or the app menu's Quit on macOS).
//! A deferred quit hides the window, and the process keeps serving. An exit
//! requested with a code (`app.exit(code)`, including the watcher below)
//! always goes through.
//!
//! The process exits once the attachment count has stayed at zero for
//! [`EXIT_GRACE`], so a live frontend that missed heartbeats (suspend, stall)
//! can re-attach first. The exit does not wait for running turns: their
//! sessions stay `busy` and the next backend start recovers them to `paused`.

use crate::server::frontend_attachments::{self, HEARTBEAT_INTERVAL, HEARTBEAT_TIMEOUT};
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, ExitRequestApi, Manager, Window, WindowEvent};

/// How long the count must stay at zero before the hidden host exits.
pub const EXIT_GRACE: Duration = HEARTBEAT_TIMEOUT;

static EXIT_WATCH_STARTED: AtomicBool = AtomicBool::new(false);

/// Whether a user-initiated quit of this process must be deferred.
pub fn should_defer_exit(hosts_backend: bool, live_attachments: usize) -> bool {
    hosts_backend && live_attachments > 0
}

/// Tracks how long the attachment count has been zero.
#[derive(Debug, Default)]
pub struct ExitWatch {
    zero_since: Option<Instant>,
}

impl ExitWatch {
    /// Record one count reading. True when the count has been zero for at
    /// least [`EXIT_GRACE`]; any live attachment resets the wait.
    pub fn should_exit(&mut self, live_attachments: usize, now: Instant) -> bool {
        if live_attachments > 0 {
            self.zero_since = None;
            return false;
        }
        let since = *self.zero_since.get_or_insert(now);
        now.saturating_duration_since(since) >= EXIT_GRACE
    }
}

fn hosts_backend() -> bool {
    crate::state::get_shared_backend_client().is_none()
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
    if defer_host_exit(window.app_handle()) {
        api.prevent_close();
    }
}

/// Run-loop hook for `RunEvent::ExitRequested`: defer a user-initiated quit
/// (no exit code) while other frontends are attached.
pub fn on_exit_requested(app: &AppHandle, code: Option<i32>, api: &ExitRequestApi) {
    if code.is_none() && defer_host_exit(app) {
        api.prevent_exit();
    }
}

/// Hide the main window and start the exit watcher when this process hosts
/// the backend and other frontends are attached. True when the quit must be
/// prevented.
fn defer_host_exit(app: &AppHandle) -> bool {
    let live = frontend_attachments::live_count();
    if !should_defer_exit(hosts_backend(), live) {
        return false;
    }
    if let Some(window) = app.get_webview_window("main") {
        if let Err(err) = window.hide() {
            warn!("Failed to hide main window on deferred exit: {err}");
        }
    }
    info!("Quit deferred with {live} attached frontend(s); backend keeps running");
    exit_when_detached(app.clone());
    true
}

fn exit_when_detached(app: AppHandle) {
    if EXIT_WATCH_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut watch = ExitWatch::default();
        loop {
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;
            if watch.should_exit(frontend_attachments::live_count(), Instant::now()) {
                info!("No frontend attached for {EXIT_GRACE:?}; shared backend exits");
                app.exit(0);
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_single_zero_reading_does_not_exit() {
        let start = Instant::now();
        let mut watch = ExitWatch::default();
        assert!(!watch.should_exit(0, start));
        assert!(!watch.should_exit(0, start + EXIT_GRACE - Duration::from_secs(1)));
        assert!(watch.should_exit(0, start + EXIT_GRACE));
    }

    #[test]
    fn a_reattached_frontend_resets_the_grace_period() {
        let start = Instant::now();
        let mut watch = ExitWatch::default();
        assert!(!watch.should_exit(0, start));
        let reattached = start + EXIT_GRACE - Duration::from_secs(1);
        assert!(!watch.should_exit(1, reattached));
        assert!(!watch.should_exit(0, start + EXIT_GRACE));
        assert!(!watch.should_exit(0, start + EXIT_GRACE + Duration::from_secs(5)));
        assert!(watch.should_exit(0, start + 2 * EXIT_GRACE));
    }
}
