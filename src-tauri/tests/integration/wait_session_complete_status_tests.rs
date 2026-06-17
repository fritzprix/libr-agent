use tauri_mcp_agent_lib::mcp::builtin::session_api::formatting::{
    is_terminal_status, is_wait_complete_status,
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
