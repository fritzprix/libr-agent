mod common;

use common::setup_test_db_with_migrations;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::service_proxy_manager::MCPServiceProxyManager;
use tauri_mcp_agent_lib::repositories::{SqliteMCPServerRepository, SqliteSessionRepository};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::{set_mcp_server_repository, set_session_repository};
use tempfile::TempDir;

#[tokio::test]
async fn reusing_builtin_only_proxy_keeps_runtime_state_sequence_stable() {
    let db = Arc::new(setup_test_db_with_migrations().await);
    set_mcp_server_repository(SqliteMCPServerRepository::new((*db).clone()));
    set_session_repository(SqliteSessionRepository::new((*db).clone()));
    let temp_dir = TempDir::new().expect("temp dir");
    let session_manager = Arc::new(
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root"))
            .expect("session manager"),
    );
    let manager = MCPServiceProxyManager::new(db, session_manager);
    let session_id = "runtime-sequence-session".to_string();

    let first_proxy = manager
        .create_proxy(session_id.clone(), Vec::new(), Vec::new(), None)
        .await
        .expect("first proxy creation should succeed");
    let first_state = manager.get_runtime_state(&session_id).await;

    assert_eq!(
        first_state.phase,
        tauri_mcp_agent_lib::agent::runtime_state::SessionRuntimePhase::Ready
    );
    assert!(
        first_state.sequence > 0,
        "initial creation should publish at least one runtime-state revision"
    );

    let reused_proxy = manager
        .create_proxy(session_id.clone(), Vec::new(), Vec::new(), None)
        .await
        .expect("reused proxy creation should succeed");
    let reused_state = manager.get_runtime_state(&session_id).await;

    assert!(
        Arc::ptr_eq(&first_proxy, &reused_proxy),
        "builtin-only reentry should reuse the existing proxy instance"
    );
    assert_eq!(
        reused_state.sequence, first_state.sequence,
        "reused create_proxy must not publish a newer runtime snapshot when nothing changed"
    );
    assert_eq!(reused_state, first_state);
}
