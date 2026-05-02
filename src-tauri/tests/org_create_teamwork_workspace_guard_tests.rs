mod common;

use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    create_org_scaffold_preflight, existing_explicit_org_identity, inspect_teamwork_scaffold,
};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::{prepare_teamwork_artifact_dir_for_session, SessionManager};

fn make_session(session_id: &str) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Org Root".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
        parent_session_id: None,
        lineage_id: Some(session_id.to_string()),
        depth: Some(0),
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
        yolo_mode: false,
        workspace_override: None,
    }
}

#[test]
fn existing_org_identity_returns_current_org_metadata() {
    let mut session = make_session("existing-org-root");
    session.org_id = Some("org-alpha".to_string());
    session.org_name = Some("Alpha Org".to_string());
    session.org_root_session_id = Some("existing-org-root".to_string());

    assert_eq!(
        existing_explicit_org_identity(&session),
        Some((
            "org-alpha".to_string(),
            "Alpha Org".to_string(),
            "existing-org-root".to_string(),
        ))
    );
}

fn extract_text(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn preparing_teamwork_artifacts_does_not_change_org_root_effective_workspace() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();
    let session_id = "org-root-session";

    repo.upsert_session(&make_session(session_id))
        .await
        .expect("session should persist");

    let original_workspace = session_manager.get_session_workspace_dir_by_id(session_id);
    let artifact_dir = prepare_teamwork_artifact_dir_for_session(&session_manager, session_id)
        .await
        .expect("artifact dir should provision");
    let effective_workspace_after = session_manager.get_session_workspace_dir_by_id(session_id);

    assert_eq!(
        original_workspace, effective_workspace_after,
        "org root should keep the same effective workspace after preparing teamwork artifacts"
    );
    assert!(
        artifact_dir.ends_with(std::path::Path::new("teamwork-artifacts").join(session_id)),
        "artifact dir should be app-local and separate from the effective workspace: {}",
        artifact_dir.display()
    );

    let persisted = repo
        .get_session(session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");
    assert_eq!(
        persisted.workspace_override, None,
        "preparing teamwork artifacts must not persist a workspace override"
    );
}

#[test]
fn create_org_preflight_rejects_missing_scaffold_with_precise_paths() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let scaffold = inspect_teamwork_scaffold(temp_dir.path());

    let result = create_org_scaffold_preflight(&scaffold)
        .expect_err("missing scaffold must block createOrg");
    let text = extract_text(&result);

    assert_eq!(result.is_error, Some(true));
    assert!(text.contains("createOrg requires a complete org teamwork scaffold"));
    assert!(text.contains(&temp_dir.path().display().to_string()));
    assert!(text.contains("agents.md"));
    assert!(text.contains("coordination/KANBAN.md"));
    assert!(text.contains(".libragent/teamwork.json"));
}
