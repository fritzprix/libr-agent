//! Frontend↔control-plane bridge for UI automation (#2902, debug builds only).
//!
//! The E2E control plane (debug builds, `LIBRAGENT_E2E_CONTROL=1`) serves UI
//! routes (`/api/e2e-control/ui/*`) that need work the webview must do:
//! enumerating interactable elements and resolving a selector to on-screen
//! coordinates. Those routes register a `request_id` here and emit Tauri
//! events; this bridge stores the frontend's answers (only for registered
//! request ids) so the HTTP handler can pick them up, and tracks how many
//! Radix dialogs the UI reports as open.
//!
//! The commands live in an always-compiled module because Tauri's
//! `invoke_handler` list cannot be conditionally compiled; every entry is
//! inert unless the control plane is enabled (debug build **and**
//! `LIBRAGENT_E2E_CONTROL=1` **and** an explicit `LIBRAGENT_DATA_DIR`), which
//! mirrors `e2e_control::token::e2e_control_enabled`. In release builds the
//! `debug_assertions` check fails and all three commands just return errors.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

const REPORT_TTL: Duration = Duration::from_secs(300);

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

#[derive(Default)]
struct BridgeStore {
    /// Request ids issued by control-plane handlers and awaiting a report.
    registered: HashMap<String, Instant>,
    /// Completed reports waiting to be taken by their handler.
    reports: HashMap<String, (Instant, Value)>,
}

type SharedBridgeStore = Mutex<BridgeStore>;

static STORE: OnceLock<SharedBridgeStore> = OnceLock::new();

fn store() -> &'static SharedBridgeStore {
    STORE.get_or_init(|| Mutex::new(BridgeStore::default()))
}

fn sweep_expired(guard: &mut BridgeStore) {
    guard
        .registered
        .retain(|_, issued| issued.elapsed() < REPORT_TTL);
    guard
        .reports
        .retain(|_, (arrived, _)| arrived.elapsed() < REPORT_TTL);
}

/// Register a `request_id` before emitting a bridge event so `put_report`
/// accepts a single answer for it (#2902 S1.2).
pub fn register_request(request_id: &str) {
    if let Ok(mut guard) = store().lock() {
        sweep_expired(&mut guard);
        guard
            .registered
            .insert(request_id.to_string(), Instant::now());
    }
}

/// Store a frontend report for `request_id` only if a control-plane handler
/// registered that id. Returns `true` when accepted, `false` when the id was
/// unknown or already answered.
pub fn put_report(request_id: &str, payload: Value) -> bool {
    let Ok(mut guard) = store().lock() else {
        return false;
    };
    sweep_expired(&mut guard);
    if guard.registered.remove(request_id).is_none() {
        return false;
    }
    guard
        .reports
        .insert(request_id.to_string(), (Instant::now(), payload));
    true
}

/// Take (and remove) the report for `request_id`, if one has arrived.
pub fn take_report(request_id: &str) -> Option<Value> {
    let mut guard = store().lock().ok()?;
    guard.reports.remove(request_id).map(|(_, payload)| payload)
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
    if !put_report(&request_id, payload) {
        return Err("unknown or already-resolved e2e request id".to_string());
    }
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
    fn put_and_take_report_round_trips_once_for_registered_id() {
        let id = "test-round-trip-1";
        register_request(id);
        assert!(put_report(id, json!({ "found": true })));
        // Second put for the same id is rejected because registration was consumed.
        assert!(!put_report(id, json!({ "found": false })));
        assert_eq!(take_report(id), Some(json!({ "found": true })));
        assert_eq!(take_report(id), None);
    }

    #[test]
    fn unregistered_request_id_is_rejected_by_put_report() {
        let id = "test-unregistered-id";
        assert!(!put_report(id, json!({ "found": true })));
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
