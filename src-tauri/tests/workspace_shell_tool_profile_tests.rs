//! Windows-safe coverage for isolation-aware workspace shell tool exposure.
//!
//! Host Windows exposes PowerShell tools only; Docker (any host) exposes bash/sh
//! tools only. Cross-dialect tools must be rejected at dispatch.

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::session::SessionManager;
use tempfile::tempdir;

fn tool_names(isolation: WorkspaceIsolationMode) -> Vec<String> {
    WorkspaceServer::tools_for_isolation(isolation)
        .into_iter()
        .map(|tool| tool.name)
        .collect()
}

fn extract_text(result: &MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn host_isolation_exposes_platform_primary_shell_only() {
    let names = tool_names(WorkspaceIsolationMode::Host);

    assert!(
        names.contains(&"spawnProcess".to_string()),
        "spawnProcess should always be present: {names:?}"
    );

    #[cfg(windows)]
    {
        assert!(names.contains(&"runPowerShell".to_string()));
        assert!(names.contains(&"runInPersistentPowerShell".to_string()));
        assert!(!names.contains(&"runShell".to_string()));
        assert!(!names.contains(&"runInPersistentShell".to_string()));
    }

    #[cfg(not(windows))]
    {
        assert!(names.contains(&"runShell".to_string()));
        assert!(names.contains(&"runInPersistentShell".to_string()));
        assert!(!names.contains(&"runPowerShell".to_string()));
        assert!(!names.contains(&"runInPersistentPowerShell".to_string()));
    }
}

#[test]
fn docker_isolation_exposes_bash_shell_tools_only() {
    let names = tool_names(WorkspaceIsolationMode::Docker);

    assert!(names.contains(&"runShell".to_string()));
    assert!(names.contains(&"runInPersistentShell".to_string()));
    assert!(names.contains(&"spawnProcess".to_string()));
    assert!(!names.contains(&"runPowerShell".to_string()));
    assert!(!names.contains(&"runInPersistentPowerShell".to_string()));
}

#[tokio::test]
async fn host_session_rejects_cross_dialect_shell_tools() {
    let temp_dir = tempdir().expect("temp dir");
    let session_manager = Arc::new(
        SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).expect("session manager"),
    );
    let server = WorkspaceServer::with_isolation(
        "shell-profile-host".to_string(),
        session_manager,
        WorkspaceIsolationMode::Host,
    );

    #[cfg(windows)]
    let unavailable = "runShell";
    #[cfg(not(windows))]
    let unavailable = "runPowerShell";

    let result = server
        .call_tool(
            unavailable,
            json!({ "command": "echo hi" }),
            Some("shell-profile-host".to_string()),
        )
        .await
        .expect("call_tool should return MCPResult");

    let text = extract_text(&result);
    assert!(
        text.contains("not available"),
        "expected not-available guidance, got: {text}"
    );
}

#[tokio::test]
async fn docker_session_rejects_powershell_tools() {
    let temp_dir = tempdir().expect("temp dir");
    let session_manager = Arc::new(
        SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).expect("session manager"),
    );
    let server = WorkspaceServer::with_isolation(
        "shell-profile-docker".to_string(),
        session_manager,
        WorkspaceIsolationMode::Docker,
    );

    let result = server
        .call_tool(
            "runPowerShell",
            json!({ "command": "Get-ChildItem" }),
            Some("shell-profile-docker".to_string()),
        )
        .await
        .expect("call_tool should return MCPResult");

    let text = extract_text(&result);
    assert!(
        text.contains("not available"),
        "expected Docker rejection of PowerShell tools, got: {text}"
    );
}
