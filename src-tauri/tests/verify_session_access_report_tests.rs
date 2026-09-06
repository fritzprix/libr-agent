//! Windows-safe coverage for verifyServer session-access reporting (#1754).
//! Standalone binary — does not pull AppHandle/WebView into the link.

use std::collections::HashSet;
use tauri_mcp_agent_lib::mcp::builtin::utils::SessionToolAccess;

fn access_with_external_ids(ids: &[&str]) -> SessionToolAccess {
    SessionToolAccess {
        session_id: Some("sess-1".to_string()),
        allowed_builtin_aliases: None,
        allowed_external_server_ids: Some(ids.iter().map(|id| (*id).to_string()).collect()),
        agent_id: Some("agent-1".to_string()),
    }
}

#[test]
fn external_access_report_unknown_without_session_context() {
    let access = SessionToolAccess {
        session_id: None,
        allowed_builtin_aliases: None,
        allowed_external_server_ids: None,
        agent_id: None,
    };

    let (status, line) = access.external_access_report(None, "srv-1", "gemini");
    assert_eq!(status, "Unknown");
    assert!(
        line.contains("unknown (no session context)"),
        "text must stay aligned with Unknown status: {line}"
    );
    assert!(
        !line.contains("[Unsupported in current session]"),
        "no-session branch must not imply a deny decision: {line}"
    );
}

#[test]
fn external_access_report_ready_when_server_attached() {
    let access = access_with_external_ids(&["srv-ready"]);
    let (status, line) = access.external_access_report(Some("sess-1"), "srv-ready", "gemini");
    assert_eq!(status, "[Ready]");
    assert!(
        line.contains("[Ready]"),
        "ready status must appear in the report line: {line}"
    );
    assert!(
        line.contains("already-running session"),
        "report must still clarify verification does not mutate the session: {line}"
    );
}

#[test]
fn external_access_report_unsupported_when_server_detached() {
    let access = access_with_external_ids(&["other-server"]);
    let (status, line) = access.external_access_report(Some("sess-1"), "srv-missing", "gemini");
    assert_eq!(status, "[Unsupported in current session]");
    assert!(
        line.contains("[Unsupported in current session]"),
        "unsupported status must appear in the report line: {line}"
    );
}

#[test]
fn external_access_report_unsupported_when_allow_list_empty() {
    let access = SessionToolAccess {
        session_id: Some("sess-1".to_string()),
        allowed_builtin_aliases: None,
        allowed_external_server_ids: Some(HashSet::new()),
        agent_id: Some("agent-1".to_string()),
    };
    let (status, _) = access.external_access_report(Some("sess-1"), "srv-1", "gemini");
    assert_eq!(status, "[Unsupported in current session]");
}
