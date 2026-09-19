//! Session id contract: display == storage; new ids are 10-hex.

use regex::Regex;
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{
    format_active_sessions_notice, SessionMetadata, SessionStatus,
};
use tauri_mcp_agent_lib::services::agent_service::spawn::generate_spawn_session_id;
use tauri_mcp_agent_lib::utils::session_id::{
    display_session_id, generate_session_id, resolve_session_id_among, SessionIdResolve,
    StorageSessionId, SESSION_ID_SHORT_LEN,
};

fn sample_session(id: &str, name: &str, status: SessionStatus) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some(name.to_string()),
        status,
        model: "gpt-test".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: Some("parent01".to_string()),
        lineage_id: Some("lineage-1".to_string()),
        depth: Some(1),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

#[test]
fn display_is_identity() {
    assert_eq!(display_session_id("a1b2c3d4e5"), "a1b2c3d4e5");
    assert_eq!(
        display_session_id("session-1735123456789012345"),
        "session-1735123456789012345"
    );
    assert_eq!(
        display_session_id("sum4n7z4fksfku0he02eoe9m"),
        "sum4n7z4fksfku0he02eoe9m"
    );
}

#[test]
fn generate_session_id_is_ten_hex() {
    let pattern = Regex::new(r"^[0-9a-f]{10}$").expect("valid regex");

    for _ in 0..20 {
        let session_id = generate_session_id();
        assert_eq!(session_id.len(), SESSION_ID_SHORT_LEN);
        assert!(pattern.is_match(&session_id), "got {session_id}");
        assert_eq!(display_session_id(&session_id), session_id);
    }

    assert_eq!(generate_spawn_session_id().len(), SESSION_ID_SHORT_LEN);
}

#[test]
fn active_sessions_notice_shows_exact_storage_ids() {
    let legacy = sample_session("session-a1b2c3d4e5", "Legacy Worker", SessionStatus::Idle);
    let modern = sample_session("a1b2c3d4e5", "Short Worker", SessionStatus::Paused);

    let notice = format_active_sessions_notice(&[legacy, modern]).expect("notice should render");

    assert!(notice.contains("`session-a1b2c3d4e5`"));
    assert!(notice.contains("`a1b2c3d4e5`"));
    assert!(notice.contains("### Sub-Agents (2)"));
}

#[test]
fn active_sessions_notice_includes_assistant_routing_identity() {
    let mut session = sample_session("codertask1", "Coder task", SessionStatus::Idle);
    session.assistant_id = Some("assistant-coder".to_string());

    let notice =
        format_active_sessions_notice(&[session]).expect("notice should render for one session");

    assert!(notice.contains("`codertask1` [config:assistant-coder] \"Coder task\""));
}

#[test]
fn storage_session_id_wraps_exact_key() {
    let typed = StorageSessionId::from_resolved("session-abc");
    assert_eq!(typed.as_str(), "session-abc");
}

#[test]
fn lookup_accepts_legacy_short_suffix_uniquely() {
    let ids = [
        "sum4n7z4fksfku0he02eoe9m",
        "session-1735123456789012345",
        "a1b2c3d4e5",
    ];
    assert_eq!(
        resolve_session_id_among(ids, "0he02eoe9m"),
        SessionIdResolve::Unique("sum4n7z4fksfku0he02eoe9m")
    );
    assert_eq!(
        resolve_session_id_among(ids, "6789012345"),
        SessionIdResolve::Unique("session-1735123456789012345")
    );
    assert_eq!(
        resolve_session_id_among(ids, "session-a1b2c3d4e5"),
        SessionIdResolve::Unique("a1b2c3d4e5")
    );
}
