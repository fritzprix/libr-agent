use crate::common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::lifecycle::repositories::init_repositories;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::parse_message_to_session_wait_config;
use tauri_mcp_agent_lib::mcp::builtin::workspace::utils::{
    default_sync_execution_timeout, max_sync_execution_timeout, resolve_sync_timeout,
};
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tempfile::tempdir;
use tokio::sync::OnceCell;
use tokio::time::{Duration, Instant};

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
fn delayed_shell_command() -> &'static str {
    "sleep 2; printf 'delayed done\\n'"
}

#[cfg(windows)]
fn delayed_shell_command() -> &'static str {
    "Start-Sleep -Seconds 2; Write-Output 'delayed done'"
}

#[test]
fn message_to_session_ignores_timeout_when_not_waiting() {
    let (wait_for_response, timeout_seconds) = parse_message_to_session_wait_config(&json!({
        "waitForResponse": false,
        "timeout": "definitely-not-a-number"
    }))
    .expect("timeout should be ignored when not waiting");

    assert!(!wait_for_response);
    assert_eq!(timeout_seconds, None);
}

#[test]
fn message_to_session_uses_default_timeout_when_waiting() {
    let (wait_for_response, timeout_seconds) = parse_message_to_session_wait_config(&json!({
        "waitForResponse": true
    }))
    .expect("waiting path should supply a default timeout");

    assert!(wait_for_response);
    assert_eq!(timeout_seconds, Some(3600));
}

#[test]
fn message_to_session_defaults_to_waiting_when_omitted() {
    let (wait_for_response, timeout_seconds) = parse_message_to_session_wait_config(&json!({}))
        .expect("waiting path should supply a default timeout when omitted");

    assert!(wait_for_response);
    assert_eq!(timeout_seconds, Some(3600));
}

#[test]
fn message_to_session_rejects_invalid_timeout_when_waiting() {
    let result = parse_message_to_session_wait_config(&json!({
        "waitForResponse": true,
        "timeout": 0
    }))
    .expect_err("invalid timeout should return MCP error");

    let text = extract_text_content(&result);
    assert!(
        text.contains("timeout must be an integer between 1 and 3600 seconds"),
        "waiting path should validate timeout bounds: {text}"
    );
}

#[test]
fn resolve_sync_timeout_caps_default_and_rejects_excessive_values() {
    assert_eq!(
        resolve_sync_timeout(None).expect("default sync timeout should be valid"),
        default_sync_execution_timeout()
    );

    let max_timeout = max_sync_execution_timeout();
    assert_eq!(
        resolve_sync_timeout(Some(max_timeout)).expect("max timeout should be allowed"),
        max_timeout
    );
    assert_eq!(
        resolve_sync_timeout(Some(max_timeout + 1)),
        Err(max_timeout),
        "timeouts above the sync limit should be rejected"
    );
}

#[tokio::test]
async fn run_shell_rejects_timeout_above_sync_limit() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "run-shell-timeout-limit";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let excessive_timeout = max_sync_execution_timeout() + 1;

    let result = server
        .handle_run_shell(
            json!({
                "command": simple_shell_command(),
                "timeout": excessive_timeout
            }),
            session_id,
        )
        .await
        .expect("runShell should return MCPResult");

    let text = extract_text_content(&result);
    assert!(
        text.contains("exceeds the sync execution limit"),
        "runShell should reject excessive timeouts: {text}"
    );
    assert!(
        text.contains("spawnProcess"),
        "runShell guidance should point to background execution: {text}"
    );
}

#[tokio::test]
async fn persistent_shell_rejects_timeout_above_sync_limit() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "persistent-shell-timeout-limit";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let excessive_timeout = max_sync_execution_timeout() + 1;

    let result = server
        .handle_execute_shell(
            json!({
                "command": simple_shell_command(),
                "timeout": excessive_timeout
            }),
            session_id,
        )
        .await
        .expect("runInPersistentShell should return MCPResult");

    let text = extract_text_content(&result);
    assert!(
        text.contains("exceeds the sync execution limit"),
        "persistent shell should reject excessive timeouts: {text}"
    );
    assert!(
        text.contains("spawnProcess"),
        "persistent shell guidance should point to background execution: {text}"
    );
}

#[tokio::test]
async fn run_shell_timeout_hands_off_to_background_process() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "run-shell-timeout-handoff";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let result = server
        .handle_run_shell(
            json!({
                "command": delayed_shell_command(),
                "timeout": 1
            }),
            session_id,
        )
        .await
        .expect("runShell should return MCPResult");

    assert_eq!(result.is_error, Some(false));

    let text = extract_text_content(&result);
    assert!(
        text.contains("still running in background"),
        "timeout handoff should be explicit in text: {text}"
    );
    assert!(
        text.contains("Process ID:"),
        "timeout handoff text should include processId: {text}"
    );
    assert!(
        text.contains("waitForProcess("),
        "timeout handoff should suggest the next wait action: {text}"
    );
    assert!(
        text.contains("readProcessOutput("),
        "timeout handoff should suggest how to inspect output: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("timeout handoff should include structured data");
    let process_id = structured["process_id"]
        .as_str()
        .expect("timeout handoff should return process_id")
        .to_string();
    assert_eq!(
        structured["execution_type"],
        json!("isolated_background_handoff")
    );
    assert!(
        matches!(
            structured["status"].as_str(),
            Some("starting") | Some("running")
        ),
        "timeout handoff should expose an active status: {}",
        structured["status"]
    );

    let wait_result = server
        .call_tool(
            "waitForProcess",
            json!({
                "processId": process_id,
                "timeout": 5
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("waitForProcess should succeed");

    let wait_structured = wait_result
        .structured_content
        .as_ref()
        .expect("waitForProcess structured content expected");
    assert_eq!(wait_structured["status"], json!("finished"));

    let output_result = server
        .call_tool(
            "readProcessOutput",
            json!({
                "processId": wait_structured["process_id"].clone(),
                "stream": "stdout",
                "mode": "tail",
                "lines": 20
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readProcessOutput should succeed");
    let output_text = extract_text_content(&output_result);
    assert!(
        output_text.contains("delayed done"),
        "background handoff should preserve process output: {output_text}"
    );
}

#[tokio::test]
async fn run_shell_success_does_not_leave_process_registry_artifacts() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "run-shell-clean-success";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let result = server
        .handle_run_shell(
            json!({
                "command": simple_shell_command(),
                "timeout": 5
            }),
            session_id,
        )
        .await
        .expect("runShell should return MCPResult");

    assert_eq!(result.is_error, Some(false));

    let listed = server
        .call_tool("listProcesses", json!({}), Some(session_id.to_string()))
        .await
        .expect("listProcesses should succeed");
    let structured = listed
        .structured_content
        .as_ref()
        .expect("listProcesses structured content expected");
    assert_eq!(structured["total"], json!(0));
}

#[tokio::test]
async fn wait_for_process_respects_timeout_budget() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "wait-process-timeout-budget";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn_result = server
        .handle_spawn_process(
            json!({
                "command": delayed_shell_command()
            }),
            session_id,
        )
        .await
        .expect("spawnProcess should succeed");

    let process_id = spawn_result
        .structured_content
        .as_ref()
        .and_then(|data| data.get("process_id"))
        .and_then(|value| value.as_str())
        .expect("spawnProcess should return process_id")
        .to_string();

    let started = Instant::now();
    let wait_result = server
        .call_tool(
            "waitForProcess",
            json!({
                "processId": process_id,
                "timeout": 1
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("waitForProcess should return MCPResult");
    let elapsed = started.elapsed();

    let text = extract_text_content(&wait_result);
    assert!(
        text.contains("Timeout waiting for process"),
        "waitForProcess should report timeout when the process keeps running: {text}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "waitForProcess exceeded the caller timeout budget too much: {:?}",
        elapsed
    );
}
