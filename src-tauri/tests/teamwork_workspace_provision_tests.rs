mod common;

use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::{
    prepare_teamwork_artifact_dir_for_session, teamwork_artifact_dir_for_session, SessionManager,
};

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
        unsafe_mode: false,
        workspace_override: None,
    }
}

#[tokio::test]
async fn prepare_teamwork_artifact_dir_uses_app_local_directory_without_persisting_override() {
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

    let artifact_dir = prepare_teamwork_artifact_dir_for_session(&session_manager, session_id)
        .await
        .expect("artifact dir should provision");

    assert!(artifact_dir.exists(), "artifact dir should exist on disk");
    assert!(
        artifact_dir.ends_with(std::path::Path::new("teamwork-artifacts").join(session_id)),
        "artifact dir should live under the app-local teamwork-artifacts root: {}",
        artifact_dir.display()
    );

    let persisted = repo
        .get_session(session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should still exist");
    assert_eq!(
        persisted.workspace_override, None,
        "preparing teamwork artifacts must not persist a workspace override"
    );

    let effective_workspace = session_manager.get_session_workspace_dir_by_id(session_id);
    assert!(
        effective_workspace.ends_with(std::path::Path::new("workspaces").join(session_id)),
        "effective workspace should stay on the normal session workspace path: {}",
        effective_workspace.display()
    );
}

#[tokio::test]
async fn prepare_teamwork_artifact_dir_is_idempotent_and_leaves_workspace_inheritance_unchanged() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();
    let session_id = "team-root-session-repeat";

    let first = prepare_teamwork_artifact_dir_for_session(&session_manager, session_id)
        .await
        .expect("first artifact-dir preparation should succeed");
    let second = prepare_teamwork_artifact_dir_for_session(&session_manager, session_id)
        .await
        .expect("second artifact-dir preparation should also succeed");

    assert_eq!(
        first, second,
        "repeated preparation should reuse the same path"
    );
    assert_eq!(
        teamwork_artifact_dir_for_session(&session_manager, session_id),
        second,
        "helper should resolve the same deterministic artifact path"
    );
    assert!(
        session_manager
            .get_session_workspace_dir_by_id(session_id)
            .ends_with(std::path::Path::new("workspaces").join(session_id)),
        "preparing artifacts must not change the effective workspace"
    );
}
