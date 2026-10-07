use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::agent::utils::{
    is_terminal_status, is_wait_complete_status, promote_settled_session_after_wait_timeout,
    should_promote_wait_timeout_to_terminal,
};

/// Regression: cancelled child sessions end in `paused`, which must unblock
/// `checkSession(wait=true)` instead of sleeping forever in the poll loop.
#[test]
fn paused_child_cancel_is_a_wait_complete_outcome() {
    assert!(
        !is_terminal_status("paused"),
        "paused must remain non-terminal for lifecycle semantics"
    );
    assert!(
        is_wait_complete_status("paused"),
        "parent wait loops must exit when a delegated session settles to paused"
    );
}

#[test]
fn wait_complete_status_preserves_terminal_set() {
    for status in ["idle", "terminated", "failed", "error"] {
        assert!(
            is_wait_complete_status(status),
            "{status} should still complete a blocking wait"
        );
    }
    assert!(
        !is_wait_complete_status("busy"),
        "busy sessions must keep the parent waiting"
    );
}

/// False-timeout race: child already idle/paused when the wait deadline hits
/// must promote to a terminal checkSession result, not a timeout payload.
#[test]
fn wait_timeout_promotes_when_child_already_settled() {
    for status in ["idle", "Idle", "paused", "error", "terminated", "failed"] {
        assert!(
            should_promote_wait_timeout_to_terminal(status),
            "{status} must promote a wait timeout to terminal delivery"
        );
    }
    for status in ["busy", "unknown", "running"] {
        assert!(
            !should_promote_wait_timeout_to_terminal(status),
            "{status} must keep the timeout error path"
        );
    }
}

#[test]
fn promote_settled_session_helper_matches_status_gate() {
    let idle = json!({ "id": "s1", "status": "idle" });
    assert!(promote_settled_session_after_wait_timeout(Some(idle)).is_some());
    let busy = json!({ "id": "s2", "status": "busy" });
    assert!(promote_settled_session_after_wait_timeout(Some(busy)).is_none());
    assert!(promote_settled_session_after_wait_timeout(None).is_none());
}
