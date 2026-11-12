//! Process readiness for session work — distinct from "process started".
//!
//! `GET /api/health` answers as soon as the HTTP server binds, which happens
//! before startup session recovery finishes. Callers that want to create or
//! drive sessions need to know the runtime is actually ready: the flag is
//! set once startup recovery completes (see `lifecycle::app_setup`).

use std::sync::atomic::{AtomicBool, Ordering};

static READY: AtomicBool = AtomicBool::new(false);

/// Mark the runtime ready to run sessions. Called once after startup
/// session recovery completes.
pub fn mark_ready() {
    READY.store(true, Ordering::Relaxed);
}

/// True once startup session recovery has completed.
pub fn is_ready() -> bool {
    READY.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_starts_false_then_sticks_true() {
        // Process-global flag: assert monotonic transitions only.
        if is_ready() {
            return;
        }
        mark_ready();
        assert!(is_ready(), "mark_ready must be visible to is_ready");
    }
}
