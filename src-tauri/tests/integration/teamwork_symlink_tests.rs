use crate::common;
use std::fs;
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::{
    ensure_session_workspace_dir, prepare_teamwork_artifact_dir_for_session, SessionManager,
};

fn make_session(
    session_id: &str,
    parent_id: Option<&str>,
    org_root_id: Option<&str>,
) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Test Session".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: parent_id.map(String::from),
        lineage_id: Some(session_id.to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: org_root_id.map(|_| "org-123".to_string()),
        org_name: org_root_id.map(|_| "Test Org".to_string()),
        org_root_session_id: org_root_id.map(String::from),
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
    }
}

#[tokio::test]
async fn test_teamwork_symlink_idempotency_and_replacement() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();

    let root_id = "root-session";
    let org_root_id = "root-session";
    let session = make_session(root_id, None, Some(org_root_id));
    repo.upsert_session(&session).await.unwrap();

    // Prepare teamwork artifacts dir on host first
    let artifact_dir = prepare_teamwork_artifact_dir_for_session(&session_manager, root_id)
        .await
        .unwrap();

    // 1. First execution: creates symlink
    let workspace_dir = ensure_session_workspace_dir(&repo, &session_manager, root_id)
        .await
        .unwrap();

    let link_path = workspace_dir.join(".libragent").join("teamwork");
    assert!(link_path.exists() || link_path.is_symlink());
    let target = fs::read_link(&link_path).unwrap();
    assert_eq!(target, artifact_dir);

    // 2. Second execution (idempotency): should not fail and should keep the symlink
    let workspace_dir_2 = ensure_session_workspace_dir(&repo, &session_manager, root_id)
        .await
        .unwrap();
    assert_eq!(workspace_dir, workspace_dir_2);
    assert!(link_path.exists() || link_path.is_symlink());
    let target_2 = fs::read_link(&link_path).unwrap();
    assert_eq!(target_2, artifact_dir);

    // 3. Re-run with wrong target path: should replace the link
    // Simulate wrong target by deleting and manually creating a wrong symlink
    fs::remove_file(&link_path).unwrap();
    let wrong_target = temp_dir.path().join("wrong-dir");
    fs::create_dir_all(&wrong_target).unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&wrong_target, &link_path).unwrap();
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("cmd");
        cmd.arg("/c")
            .arg("mklink")
            .arg("/j")
            .arg(&link_path)
            .arg(&wrong_target);
        cmd.output().unwrap();
    }

    assert_eq!(fs::read_link(&link_path).unwrap(), wrong_target);

    // Re-run workspace setup. It should detect the wrong target and replace it with the correct one.
    ensure_session_workspace_dir(&repo, &session_manager, root_id)
        .await
        .unwrap();

    assert_eq!(fs::read_link(&link_path).unwrap(), artifact_dir);
}

#[tokio::test]
async fn test_non_org_root_symlink_resolution() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();

    let root_id = "non-org-root";
    // Non-org root session
    let session = make_session(root_id, None, None);
    repo.upsert_session(&session).await.unwrap();

    // Prepare teamwork artifacts dir
    let artifact_dir = prepare_teamwork_artifact_dir_for_session(&session_manager, root_id)
        .await
        .unwrap();

    let workspace_dir = ensure_session_workspace_dir(&repo, &session_manager, root_id)
        .await
        .unwrap();

    let link_path = workspace_dir.join(".libragent").join("teamwork");
    assert!(link_path.exists() || link_path.is_symlink());
    assert_eq!(fs::read_link(&link_path).unwrap(), artifact_dir);
}

#[tokio::test]
async fn test_org_child_inherits_parent_teamwork_root() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();

    let root_id = "parent-root";
    let child_id = "child-session";
    let org_root_id = "parent-root";

    let parent_session = make_session(root_id, None, Some(org_root_id));
    let child_session = make_session(child_id, Some(root_id), Some(org_root_id));
    repo.upsert_session(&parent_session).await.unwrap();
    repo.upsert_session(&child_session).await.unwrap();

    // Prepare teamwork folder under parent root ID
    let parent_artifact_dir = prepare_teamwork_artifact_dir_for_session(&session_manager, root_id)
        .await
        .unwrap();

    // Hydrate child session's workspace override or normal workspace
    let child_workspace_dir = ensure_session_workspace_dir(&repo, &session_manager, child_id)
        .await
        .unwrap();

    let link_path = child_workspace_dir.join(".libragent").join("teamwork");
    assert!(link_path.exists() || link_path.is_symlink());
    // Child symlink must point to the parent's teamwork folder!
    assert_eq!(fs::read_link(&link_path).unwrap(), parent_artifact_dir);
}

#[tokio::test]
async fn test_workspace_server_path_resolution_and_security() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");

    let base_dir = temp_dir.path().join("session-root");
    let session_manager = SessionManager::new_with_base_dir(base_dir.clone()).unwrap();
    let session_id = "test-security-session";
    let org_root_id = "test-security-session";

    let session = make_session(session_id, None, Some(org_root_id));
    repo.upsert_session(&session).await.unwrap();

    let artifact_dir = prepare_teamwork_artifact_dir_for_session(&session_manager, session_id)
        .await
        .unwrap();

    let _workspace_dir = ensure_session_workspace_dir(&repo, &session_manager, session_id)
        .await
        .unwrap();

    // Create a mock kanban file inside the teamwork directory
    let tw_coordination_dir = artifact_dir.join("coordination");
    fs::create_dir_all(&tw_coordination_dir).unwrap();
    let kanban_file = tw_coordination_dir.join("KANBAN.md");
    fs::write(&kanban_file, "Backlog").unwrap();

    // Instantiate WorkspaceServer
    use std::sync::Arc;
    use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
    let server = WorkspaceServer::new(session_id.to_string(), Arc::new(session_manager));

    // 1. Resolve relative teamwork path via workspace server
    let resolved = server
        .validate_read_path_with_skill_access(".libragent/teamwork/coordination/KANBAN.md", None)
        .await
        .unwrap();
    println!("RESOLVED PATH: {:?}", resolved);
    println!("KANBAN_FILE PATH: {:?}", kanban_file);
    assert_eq!(
        resolved.canonicalize().unwrap(),
        kanban_file.canonicalize().unwrap()
    );

    // 2. Resolve virtual teamwork path
    let resolved_virtual = server
        .validate_read_path_with_skill_access("@teamwork/coordination/KANBAN.md", None)
        .await
        .unwrap();
    assert_eq!(
        resolved_virtual.canonicalize().unwrap(),
        kanban_file.canonicalize().unwrap()
    );

    // 3. Test path traversal security regression (should fail or resolve within teamwork dir)
    let traversal_res = server
        .validate_read_path_with_skill_access(".libragent/teamwork/../../outside.txt", None)
        .await;
    assert!(
        traversal_res.is_err(),
        "Path traversal outside teamwork root must fail security check"
    );
}
