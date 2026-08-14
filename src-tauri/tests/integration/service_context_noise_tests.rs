use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::attachments::AttachmentsServer;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::session::SessionManager;
use tempfile::tempdir;

use crate::common;
use serde_json::json;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::lifecycle::repositories::init_repositories;
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tokio::sync::OnceCell;

fn build_session_manager(base_dir: &std::path::Path) -> Arc<SessionManager> {
    Arc::new(SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager"))
}

async fn ensure_settings_repository() {
    static REPOSITORIES: OnceCell<()> = OnceCell::const_new();

    REPOSITORIES
        .get_or_init(|| async {
            let db = common::setup_test_db_with_migrations().await;
            init_repositories(&db).await;
            init_session_bus(SessionBus::new());
            init_concurrency_gate(ConcurrencyGate::new(
                DEFAULT_MAX_ACTIVE_AGENTS,
                DEFAULT_MAX_SUSPENDED_AGENTS,
                DEFAULT_MAX_ACTIVE_PROCESSES,
                DEFAULT_MAX_SUSPENDED_PROCESSES,
            ));
        })
        .await;
}

#[cfg(unix)]
fn delayed_shell_command() -> &'static str {
    "sleep 2; printf 'delayed done\\n'"
}

#[cfg(windows)]
fn delayed_shell_command() -> &'static str {
    "Start-Sleep -Seconds 2; Write-Output 'delayed done'"
}

#[tokio::test]
async fn workspace_service_context_exposes_platform_and_drops_process_guidance() {
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
    // Platform/shell are intentional agent-facing live state (not noise).
    assert!(service_context.context_prompt.contains("- Platform:"));
    assert!(service_context.context_prompt.contains("- Default Shell:"));
    assert!(!service_context.context_prompt.contains("waitForProcess"));
}

/// End-to-end lifecycle: spawn → context lists process → finish → context shows None.
/// Runs only inside the non-Windows consolidated integration binary.
#[tokio::test]
async fn workspace_service_context_running_processes_track_lifecycle() {
    ensure_settings_repository().await;

    let temp = tempdir().expect("tempdir");
    let session_id = "workspace-context-process-lifecycle";
    let server = WorkspaceServer::new(session_id.to_string(), build_session_manager(temp.path()));

    let empty = server.get_service_context(None).await;
    assert!(
        empty.context_prompt.contains("- Running Processes: None"),
        "expected empty running processes: {}",
        empty.context_prompt
    );

    let spawn_result = server
        .handle_spawn_process(json!({ "command": delayed_shell_command() }), session_id)
        .await
        .expect("spawnProcess should succeed");
    let process_id = spawn_result
        .structured_content
        .as_ref()
        .and_then(|data| data.get("process_id"))
        .and_then(|value| value.as_str())
        .expect("spawnProcess should return process_id")
        .to_string();

    let while_running = server.get_service_context(None).await;
    assert!(
        while_running.context_prompt.contains(&process_id),
        "running process must appear in service context: {}",
        while_running.context_prompt
    );
    assert!(
        !while_running
            .context_prompt
            .contains("- Running Processes: None"),
        "must not report None while a process is active: {}",
        while_running.context_prompt
    );

    let wait_result = server
        .call_tool(
            "waitForProcess",
            json!({ "processId": process_id, "timeout": 10 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("waitForProcess should succeed");
    let status = wait_result
        .structured_content
        .as_ref()
        .and_then(|data| data.get("status"))
        .and_then(|value| value.as_str())
        .expect("waitForProcess should return status");
    assert!(
        matches!(status, "finished" | "failed" | "killed"),
        "process did not finish: {status}"
    );

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let after_finish = server.get_service_context(None).await;
    assert!(
        after_finish
            .context_prompt
            .contains("- Running Processes: None"),
        "finished process must leave Running Processes as None: {}",
        after_finish.context_prompt
    );
    assert!(
        !after_finish.context_prompt.contains(&process_id),
        "finished process id must not remain listed as running: {}",
        after_finish.context_prompt
    );
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
    assert!(service_context
        .context_prompt
        .contains("No files attached. Use `attachments__upload` to add files."));
    assert!(!service_context.context_prompt.contains("Attachments: None"));
    assert!(!service_context
        .context_prompt
        .contains("No attachments available yet"));
    assert!(!service_context.context_prompt.contains("Use `read("));
}
