use crate::common;

use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};

#[tokio::test]
async fn session_repository_persists_explicit_org_identity() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);

    repo.upsert_session(&SessionMetadata {
        id: "org-root-session".to_string(),
        name: Some("Org Root".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: Some("org-root-session".to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: Some("org-alpha".to_string()),
        org_name: Some("Alpha Org".to_string()),
        org_root_session_id: Some("org-root-session".to_string()),
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation:
            tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    })
    .await
    .expect("session should persist");

    let loaded = repo
        .get_session("org-root-session")
        .await
        .expect("lookup should succeed")
        .expect("session should exist");

    assert_eq!(loaded.org_id.as_deref(), Some("org-alpha"));
    assert_eq!(loaded.org_name.as_deref(), Some("Alpha Org"));
    assert_eq!(
        loaded.org_root_session_id.as_deref(),
        Some("org-root-session")
    );
}
