mod common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::lifecycle::repositories::init_repositories;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tempfile::tempdir;
use tokio::sync::OnceCell;

fn extract_text_content(result: &MCPResult) -> String {
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

fn build_workspace_server(base_dir: &std::path::Path, session_id: &str) -> WorkspaceServer {
    let session_manager =
        SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager");
    WorkspaceServer::new(session_id.to_string(), Arc::new(session_manager))
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
fn simple_shell_command() -> &'static str {
    "printf 'hello\\n'"
}

#[cfg(windows)]
fn simple_shell_command() -> &'static str {
    "Write-Output 'hello'"
}

#[cfg(unix)]
fn move_into_subdir_command() -> &'static str {
    "mkdir -p sandbox && cd sandbox && pwd"
}

#[cfg(windows)]
fn move_into_subdir_command() -> &'static str {
    "New-Item -ItemType Directory -Path sandbox -Force | Out-Null; Set-Location sandbox; Get-Location"
}

#[tokio::test]
async fn read_file_out_of_range_guidance_points_to_line_bounds() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-out-of-range-guidance";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "a\nb\nc\nd\ne\n").expect("write file");

    let result = server
        .handle_read_file(
            json!({
                "path": "sample.txt",
                "startLine": 100
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return MCPResult");

    let text = extract_text_content(&result);
    assert!(
        text.contains("Choose startLine between 1 and 5 for this file"),
        "out-of-range guidance should point at valid line bounds: {text}"
    );
    assert!(
        !text.contains("Check file permissions"),
        "out-of-range guidance should not send the agent down a permission rabbit hole: {text}"
    );
}

#[tokio::test]
async fn list_directory_empty_response_does_not_suggest_rerunning_same_call() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "list-directory-empty-guidance";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join("empty")).expect("create empty dir");

    let result = server
        .handle_list_directory(
            json!({
                "path": "empty"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("listDirectory should return MCPResult");

    let text = extract_text_content(&result);
    assert!(
        !text.contains("Use listDirectory with {\"path\": \"empty\"}"),
        "empty directory guidance should not recommend rerunning the same listDirectory call: {text}"
    );
    assert!(
        !text.contains("💡 Next Steps:"),
        "listDirectory should not duplicate next-step rendering in the message body: {text}"
    );
}

#[tokio::test]
async fn run_shell_success_hint_no_longer_mentions_stop_process() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "run-shell-guidance";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let result = server
        .handle_run_shell(
            json!({
                "command": simple_shell_command()
            }),
            session_id,
        )
        .await
        .expect("runShell should succeed");

    let text = extract_text_content(&result);
    assert!(
        !text.contains("stopProcess"),
        "completed runShell responses should not mention stopProcess: {text}"
    );
}

#[tokio::test]
async fn persistent_shell_uses_shell_specific_guidance_after_changing_cwd() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "persistent-shell-guidance";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let result = server
        .handle_execute_shell(
            json!({
                "command": move_into_subdir_command()
            }),
            session_id,
        )
        .await
        .expect("runInPersistentShell should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("readFile and listDirectory still use workspace root, not the shell CWD"),
        "persistent shell guidance should explicitly warn about workspace-root file tools: {text}"
    );
    assert!(
        !text.contains("Use listDirectory to verify file system changes"),
        "persistent shell guidance should not recommend listDirectory for shell-local CWD changes: {text}"
    );
}
