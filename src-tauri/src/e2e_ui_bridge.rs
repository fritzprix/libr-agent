//! Frontend↔control-plane bridge for UI automation (#2902, debug builds only).
//!
//! The E2E control plane (debug builds, `LIBRAGENT_E2E_CONTROL=1`) serves UI
//! routes (`/api/e2e-control/ui/*`) that need work the webview must do:
//! enumerating interactable elements and resolving a selector to on-screen
//! coordinates. Those routes emit Tauri events; this bridge stores the
//! frontend's answers here (keyed by request id) so the HTTP handler can pick
//! them up, and tracks how many Radix dialogs the UI reports as open.
//!
//! The commands live in an always-compiled module because Tauri's
//! `invoke_handler` list cannot be conditionally compiled; every entry is
//! inert unless the control plane is enabled (debug build **and**
//! `LIBRAGENT_E2E_CONTROL=1` **and** an explicit `LIBRAGENT_DATA_DIR`), which
//! mirrors `e2e_control::token::e2e_control_enabled`. In release builds the
//! debug_assertions check fails and all three commands just return errors.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

/// True when the UI bridge may carry data: debug build, explicit opt-in env,
/// and explicit data dir. Mirrors the control plane's enablement test so a
/// release (or non-opt-in) process never stores a report.
pub fn ui_bridge_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var("LIBRAGENT_E2E_CONTROL").ok().as_deref() == Some("1")
        && std::env::var_os("LIBRAGENT_DATA_DIR")
            .filter(|value| !value.is_empty())
            .is_some()
}

/// One frontend answer, with the instant it arrived so stale entries can be
/// swept even when the waiting handler already timed out.
type ReportStore = Mutex<HashMap<String, (Instant, Value)>>;

static REPORTS: OnceLock<ReportStore> = OnceLock::new();

fn reports() -> &'static ReportStore {
    REPORTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store a frontend report for `request_id`, sweeping entries older than five
/// minutes first (a handler that gave up never comes back for its report).
pub fn put_report(request_id: &str, payload: Value) {
    if let Ok(mut store) = reports().lock() {
        store.retain(|_, (arrived, _)| arrived.elapsed() < Duration::from_secs(300));
        store.insert(request_id.to_string(), (Instant::now(), payload));
    }
}

/// Take (and remove) the report for `request_id`, if one has arrived.
pub fn take_report(request_id: &str) -> Option<Value> {
    reports()
        .lock()
        .ok()?
        .remove(request_id)
        .map(|(_, payload)| payload)
}

/// How many shared-`Dialog` instances the frontend currently reports open.
static DIALOG_OPEN_COUNT: AtomicI64 = AtomicI64::new(0);

fn adjust_dialog_count(open: bool) {
    let delta = if open { 1 } else { -1 };
    let next = DIALOG_OPEN_COUNT.fetch_add(delta, Ordering::Relaxed) + delta;
    if next < 0 {
        // A close arrived without a matching open (e.g. the webview reloaded
        // mid-dialog). Clamp rather than let the counter drift negative.
        DIALOG_OPEN_COUNT.store(0, Ordering::Relaxed);
    }
}

/// True while at least one shared dialog is open.
pub fn dialog_open() -> bool {
    DIALOG_OPEN_COUNT.load(Ordering::Relaxed) > 0
}

/// Frontend probe: only register the bridge listeners when the control plane
/// is actually enabled in this process.
#[tauri::command]
pub fn e2e_ui_enabled() -> bool {
    ui_bridge_enabled()
}

/// Report a request-scoped payload (inspect / target results) from the
/// frontend back to the waiting control-plane handler.
#[tauri::command]
pub fn e2e_ui_report(request_id: String, payload: Value) -> Result<(), String> {
    if !ui_bridge_enabled() {
        return Err("e2e control plane is disabled".to_string());
    }
    put_report(&request_id, payload);
    Ok(())
}

/// Track shared-dialog open/close transitions. `reset = true` zeroes the
/// counter (the frontend calls it once when the bridge mounts, so a reload
/// cannot leave a stale open dialog behind).
#[tauri::command]
pub fn e2e_ui_report_dialog(open: bool, reset: Option<bool>) -> Result<(), String> {
    if !ui_bridge_enabled() {
        return Err("e2e control plane is disabled".to_string());
    }
    if reset.unwrap_or(false) {
        DIALOG_OPEN_COUNT.store(0, Ordering::Relaxed);
    } else {
        adjust_dialog_count(open);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn put_and_take_report_round_trips_once() {
        // These run in the same process as other tests; unique ids keep them
        // isolated without needing a lock around the global store.
        let id = "test-round-trip-1";
        put_report(id, json!({ "found": true }));
        assert_eq!(take_report(id), Some(json!({ "found": true })));
        assert_eq!(take_report(id), None);
    }

    #[test]
    fn take_report_of_unknown_id_is_none() {
        assert_eq!(take_report("test-never-put"), None);
    }

    #[test]
    fn dialog_counter_clamps_at_zero() {
        DIALOG_OPEN_COUNT.store(0, Ordering::Relaxed);
        adjust_dialog_count(false); // close without open
        assert_eq!(DIALOG_OPEN_COUNT.load(Ordering::Relaxed), 0);
        assert!(!dialog_open());
        adjust_dialog_count(true);
        assert!(dialog_open());
        adjust_dialog_count(true);
        adjust_dialog_count(false);
        adjust_dialog_count(false);
        assert!(!dialog_open());
        DIALOG_OPEN_COUNT.store(0, Ordering::Relaxed);
    }
}
