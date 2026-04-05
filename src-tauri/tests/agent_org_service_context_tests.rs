mod common;

use std::sync::Arc;

use tauri_mcp_agent_lib::mcp::builtin::agent::AgentServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};

fn build_session(
    id: &str,
    name: &str,
    parent_session_id: Option<&str>,
    depth: Option<u32>,
    org_id: Option<&str>,
    org_name: Option<&str>,
    org_root_session_id: Option<&str>,
) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some(name.to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
        parent_session_id: parent_session_id.map(str::to_string),
        lineage_id: Some("lineage-1".to_string()),
        depth,
        max_depth: None,
        max_fanout: None,
        org_id: org_id.map(str::to_string),
        org_name: org_name.map(str::to_string),
        org_root_session_id: org_root_session_id.map(str::to_string),
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        yolo_mode: false,
        workspace_override: None,
    }
}

#[tokio::test]
async fn org_service_context_includes_only_local_org_layer_for_org_sessions() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    repo.upsert_session(&build_session(
        "root",
        "Root Coordinator",
        None,
        Some(0),
        Some("org-alpha"),
        Some("Alpha Org"),
        Some("root"),
    ))
    .await
    .expect("root session should persist");
    repo.upsert_session(&build_session(
        "child-a",
        "Analyst",
        Some("root"),
        Some(1),
        Some("org-alpha"),
        Some("Alpha Org"),
        Some("root"),
    ))
    .await
    .expect("child session should persist");
    repo.upsert_session(&build_session(
        "child-b",
        "Writer",
        Some("root"),
        Some(1),
        Some("org-alpha"),
        Some("Alpha Org"),
        Some("root"),
    ))
    .await
    .expect("sibling session should persist");
    repo.upsert_session(&build_session(
        "other-depth",
        "Reviewer",
        Some("child-a"),
        Some(2),
        Some("org-alpha"),
        Some("Alpha Org"),
        Some("root"),
    ))
    .await
    .expect("different depth session should persist");

    let server = AgentServer::new("child-a".to_string(), Arc::new(db), None)
        .await
        .expect("agent server should initialize");

    let context = server.get_service_context(None).await;

    assert!(
        context.context_prompt.contains("## Explicit Org Layer"),
        "org sessions should get an explicit org layer section"
    );
    assert!(context.context_prompt.contains("- Org: Alpha Org"));
    assert!(context.context_prompt.contains("- Depth: 1"));
    assert!(context
        .context_prompt
        .contains("- Parent: root — Root Coordinator"));
    assert!(
        context.context_prompt.contains("  - child-b — Writer"),
        "same-depth sibling should appear in local org layer"
    );
    assert!(
        !context.context_prompt.contains("other-depth"),
        "deeper descendants must not leak into the local org layer"
    );
    assert!(
        !context.context_prompt.contains("None"),
        "prompt should not contain useless placeholder values"
    );
}

#[tokio::test]
async fn org_service_context_omits_org_section_for_non_org_sessions() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    repo.upsert_session(&build_session(
        "solo",
        "Solo Session",
        None,
        Some(0),
        None,
        None,
        None,
    ))
    .await
    .expect("solo session should persist");

    let server = AgentServer::new("solo".to_string(), Arc::new(db), None)
        .await
        .expect("agent server should initialize");

    let context = server.get_service_context(None).await;

    assert!(
        !context.context_prompt.contains("## Explicit Org Layer"),
        "non-org sessions should not receive an org section"
    );
}
