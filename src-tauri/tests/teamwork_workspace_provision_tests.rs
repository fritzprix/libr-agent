mod common;

use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::{provision_teamwork_workspace_for_session, SessionManager};

fn make_session(session_id: &str) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Team Root".to_string()),
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

#[tokio::test]
async fn provision_teamwork_workspace_uses_dedicated_directory_and_persists_override() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();
    let session_id = "team-root-session";

    repo.upsert_session(&make_session(session_id))
        .await
        .expect("session should persist");

    let teamwork_workspace =
        provision_teamwork_workspace_for_session(&repo, &session_manager, session_id)
            .await
            .expect("teamwork workspace should provision");

    assert!(
        teamwork_workspace.exists(),
        "workspace should exist on disk"
    );
    assert!(
        teamwork_workspace.ends_with(std::path::Path::new("teamwork-workspaces").join(session_id)),
        "teamwork workspace should live under the dedicated teamwork-workspaces root: {}",
        teamwork_workspace.display()
    );

    let persisted = repo
        .get_session(session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should still exist");
    assert_eq!(
        persisted.workspace_override.as_deref(),
        teamwork_workspace.to_str(),
        "workspace override should persist to the repository"
    );

    let effective_workspace = session_manager.get_session_workspace_dir_by_id(session_id);
    assert_eq!(
        effective_workspace, teamwork_workspace,
        "session manager should resolve the teamwork workspace as the effective workspace"
    );
}

#[tokio::test]
async fn provision_teamwork_workspace_is_idempotent_for_repeated_calls() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();
    let session_id = "team-root-session-repeat";

    repo.upsert_session(&make_session(session_id))
        .await
        .expect("session should persist");

    let first = provision_teamwork_workspace_for_session(&repo, &session_manager, session_id)
        .await
        .expect("first teamwork workspace provisioning should succeed");
    let second = provision_teamwork_workspace_for_session(&repo, &session_manager, session_id)
        .await
        .expect("second teamwork workspace provisioning should also succeed");

    assert_eq!(
        first, second,
        "repeated provisioning should reuse the same path"
    );

    let persisted = repo
        .get_session(session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");
    assert_eq!(persisted.workspace_override.as_deref(), second.to_str());
    assert_eq!(
        session_manager.get_session_workspace_dir_by_id(session_id),
        second,
        "session manager should still resolve the same teamwork workspace after repeated provisioning"
    );
}
