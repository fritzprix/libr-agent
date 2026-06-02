use tauri_mcp_agent_lib::services::agent_service::resolve_workspace_override_path;
use tauri_mcp_agent_lib::session::SessionManager;
use tempfile::tempdir;

#[tokio::test]
async fn resolve_workspace_override_path_canonicalizes_dot_segments() {
    let temp_dir = tempdir().expect("temp dir should be created");
    let nested_dir = temp_dir.path().join("nested");
    std::fs::create_dir_all(&nested_dir).expect("nested dir should be created");

    let path_with_dot_segments = nested_dir.join("..").join("nested");
    let resolved = resolve_workspace_override_path(
        path_with_dot_segments
            .to_str()
            .expect("temp path should be valid utf-8"),
    )
    .await
    .expect("workspace path should resolve");

    assert_eq!(
        resolved,
        std::fs::canonicalize(&nested_dir).expect("nested dir should canonicalize")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn resolve_workspace_override_path_rejects_restricted_traversal() {
    let error = resolve_workspace_override_path("/tmp/../etc")
        .await
        .expect_err("restricted path traversal should be rejected");

    assert!(error.contains("restricted system directory"));
}

#[cfg(unix)]
#[tokio::test]
async fn resolve_workspace_override_path_rejects_symlinked_restricted_directory() {
    use std::os::unix::fs::symlink;

    let temp_dir = tempdir().expect("temp dir should be created");
    let symlink_path = temp_dir.path().join("system-link");
    symlink("/etc", &symlink_path).expect("symlink should be created");

    let error = resolve_workspace_override_path(
        symlink_path
            .to_str()
            .expect("symlink path should be valid utf-8"),
    )
    .await
    .expect_err("symlinked restricted directory should be rejected");

    assert!(error.contains("restricted system directory"));
}

#[tokio::test]
async fn registered_workspace_override_cleanup_removes_precreated_session_entry() {
    let base_dir = tempdir().expect("base temp dir should be created");
    let override_dir = tempdir().expect("override temp dir should be created");
    let session_manager = SessionManager::new_with_base_dir(base_dir.path().to_path_buf())
        .expect("session manager should initialize");
    let session_id = "spawn-cleanup-session";

    session_manager
        .register_session_override(session_id, override_dir.path().to_path_buf())
        .await
        .expect("workspace override should register");

    assert_eq!(
        session_manager
            .get_session_stats()
            .expect("session stats should load")
            .total_sessions,
        1
    );
    assert_eq!(
        session_manager.get_session_workspace_dir_by_id(session_id),
        override_dir.path().to_path_buf()
    );

    session_manager
        .remove_session(session_id)
        .await
        .expect("precreated session entry should be removable");

    assert_eq!(
        session_manager
            .get_session_stats()
            .expect("session stats should load")
            .total_sessions,
        0
    );
    assert!(
        !base_dir.path().join("workspaces").join(session_id).exists(),
        "cleanup should remove the default precreated workspace directory"
    );
}
