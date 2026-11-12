//! Process readiness for session work — distinct from "process started".
//!
//! `GET /api/health` answers as soon as the HTTP server binds, which happens
//! before startup session recovery finishes. Callers that want to create or
//! drive sessions need to know the runtime is actually ready: the flag is
//! set once startup recovery completes (see `lifecycle::app_setup`).

use std::sync::atomic::{AtomicU8, Ordering};

/// What happened to startup session recovery in this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecoveryState {
    /// Still running; the runtime is not ready yet.
    Pending,
    /// Recovery finished.
    Ok,
    /// Recovery returned an error. Sessions may still be marked `Busy`.
    Failed,
    /// Another live backend owns the sessions, so this process left them alone.
    Skipped,
}

static RECOVERY: AtomicU8 = AtomicU8::new(RecoveryState::Pending as u8);

/// Record how startup recovery ended. Any outcome makes the runtime ready:
/// `/api/health` must not stay at `ready: false` forever, and the `recovery`
/// field tells a client when the outcome was not clean.
pub fn mark_recovery(state: RecoveryState) {
    RECOVERY.store(state as u8, Ordering::Relaxed);
}

pub fn recovery_state() -> RecoveryState {
    match RECOVERY.load(Ordering::Relaxed) {
        value if value == RecoveryState::Ok as u8 => RecoveryState::Ok,
        value if value == RecoveryState::Failed as u8 => RecoveryState::Failed,
        value if value == RecoveryState::Skipped as u8 => RecoveryState::Skipped,
        _ => RecoveryState::Pending,
    }
}

/// True once startup session recovery has ended, whatever its outcome.
pub fn is_ready() -> bool {
    recovery_state() != RecoveryState::Pending
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_once_recovery_has_an_outcome() {
        // Process-global state shared with other tests: only assert the rule.
        let before = recovery_state();
        assert_eq!(is_ready(), before != RecoveryState::Pending);

        mark_recovery(RecoveryState::Failed);
        assert!(is_ready(), "a failed recovery still ends the wait");
        assert_eq!(recovery_state(), RecoveryState::Failed);

        mark_recovery(RecoveryState::Ok);
        assert_eq!(recovery_state(), RecoveryState::Ok);
    }

    #[test]
    fn recovery_state_serializes_lowercase() {
        assert_eq!(
            serde_json::to_value(RecoveryState::Skipped).unwrap(),
            serde_json::json!("skipped")
        );
    }
}
