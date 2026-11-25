//! Test hook for native file dialogs (E2E control plane, #2903).
//!
//! Native open/save dialogs block unattended harnesses: nobody is there to
//! pick a path. The debug-only control plane route
//! `POST /api/e2e-control/picker` stores one *override* here; the next dialog
//! (frontend wrapper or the Rust save path) consumes it instead of showing a
//! real dialog.
//!
//! The store is inert unless something sets an override. The only writer is
//! the capability-guarded, loopback-only, debug-builds-only control plane, so
//! release builds and normal production runs never see an override. The
//! command is compiled in all builds and always answers `None` when the
//! control plane is unavailable, so the frontend wrapper needs no
//! build-dependent special case.

use std::sync::Mutex;

/// What the harness resolved for the next native dialog.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum PickerOverride {
    /// Choose this path (file or directory, as the dialog asked).
    Path { path: String },
    /// Press "Cancel".
    Cancel,
}

static OVERRIDE: Mutex<Option<PickerOverride>> = Mutex::new(None);

/// Store the override for the next dialog. An override set before the previous
/// one was consumed is replaced (the harness, not the app, decides).
pub fn set_picker_override(override_: PickerOverride) {
    let mut slot = OVERRIDE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = Some(override_);
}

/// Consume the override for the next dialog, if any (one shot).
pub fn take_picker_override() -> Option<PickerOverride> {
    let mut slot = OVERRIDE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    slot.take()
}

/// Frontend wrapper probe: the pending picker override, if the control plane
/// armed one. Consumes it. Always `None` in release / without opt-in because
/// nothing can set an override there.
#[tauri::command]
pub fn e2e_take_picker_override() -> Option<PickerOverride> {
    take_picker_override()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The store is a process-wide singleton, so these tests share it. Keep
    // them serialized on one thread via a mutex to avoid cross-test steals.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn take_with_lock() -> Option<PickerOverride> {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        take_picker_override()
    }

    #[test]
    fn take_is_none_when_nothing_was_set() {
        assert_eq!(take_with_lock(), None);
    }

    #[test]
    fn set_then_take_returns_the_override_once() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_picker_override(PickerOverride::Path {
            path: "/tmp/e2e-target.txt".to_string(),
        });
        assert_eq!(
            take_picker_override(),
            Some(PickerOverride::Path {
                path: "/tmp/e2e-target.txt".to_string()
            })
        );
        assert_eq!(take_picker_override(), None);
    }

    #[test]
    fn newer_override_replaces_the_older_one() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_picker_override(PickerOverride::Path {
            path: "/old".to_string(),
        });
        set_picker_override(PickerOverride::Cancel);
        assert_eq!(take_picker_override(), Some(PickerOverride::Cancel));
        assert_eq!(take_picker_override(), None);
    }
}
