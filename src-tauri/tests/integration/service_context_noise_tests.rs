use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::attachments::AttachmentsServer;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::session::SessionManager;
use tempfile::tempdir;

fn build_session_manager(base_dir: &std::path::Path) -> Arc<SessionManager> {
    Arc::new(SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager"))
}

#[tokio::test]
async fn workspace_service_context_drops_platform_and_process_guidance() {
    let temp = tempdir().expect("tempdir");
    let server = WorkspaceServer::new(
        "workspace-noise-test".to_string(),
        build_session_manager(temp.path()),
    );

    let service_context = server.get_service_context(None).await;

    assert!(service_context.context_prompt.contains("## Workspace"));
    assert!(service_context.context_prompt.contains("- Workspace Root:"));
    assert!(service_context
        .context_prompt
        .contains("- Persistent Shell CWD:"));
    assert!(!service_context.context_prompt.contains("- Platform:"));
    assert!(!service_context
        .context_prompt
        .contains("Use waitForProcess"));
}

#[tokio::test]
async fn attachments_service_context_uses_compact_empty_state() {
    let temp = tempdir().expect("tempdir");
    let server = AttachmentsServer::new(
        "attachments-noise-test".to_string(),
        build_session_manager(temp.path()),
    );

    let service_context = server.get_service_context(None).await;

    assert!(service_context.context_prompt.contains("## Attachments"));
    assert!(service_context.context_prompt.contains("Attachments: None"));
    assert!(!service_context
        .context_prompt
        .contains("No attachments available yet"));
    assert!(!service_context.context_prompt.contains("Use `read("));
}
