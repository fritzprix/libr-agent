use std::collections::HashMap;

use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::is_delegated_descendant_session;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};

fn session(id: &str, parent_session_id: Option<&str>) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some(id.to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
        parent_session_id: parent_session_id.map(str::to_string),
        lineage_id: Some("root".to_string()),
        depth: Some(match parent_session_id {
            None => 0,
            Some("root") => 1,
            _ => 2,
        }),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 0,
        updated_at: 0,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        yolo_mode: false,
        workspace_override: None,
    }
}

#[test]
fn descendant_access_allows_nested_child_sessions() {
    let sessions = HashMap::from([
        ("root".to_string(), session("root", None)),
        ("child-a".to_string(), session("child-a", Some("root"))),
        (
            "grandchild-a1".to_string(),
            session("grandchild-a1", Some("child-a")),
        ),
    ]);

    assert!(is_delegated_descendant_session(
        &sessions,
        "root",
        "grandchild-a1"
    ));
    assert!(is_delegated_descendant_session(
        &sessions,
        "child-a",
        "grandchild-a1"
    ));
}

#[test]
fn descendant_access_rejects_self_and_sibling_sessions() {
    let sessions = HashMap::from([
        ("root".to_string(), session("root", None)),
        ("child-a".to_string(), session("child-a", Some("root"))),
        ("child-b".to_string(), session("child-b", Some("root"))),
    ]);

    assert!(!is_delegated_descendant_session(
        &sessions, "child-a", "child-a"
    ));
    assert!(!is_delegated_descendant_session(
        &sessions, "child-a", "child-b"
    ));
    assert!(!is_delegated_descendant_session(
        &sessions, "child-b", "child-a"
    ));
}
