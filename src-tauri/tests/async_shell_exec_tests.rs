//! Integration tests for async shell execution (`execute_shell_async`) (#2907).
//!
//! Exercises `WorkspaceServer::handle_spawn_process`, `waitForProcess` (polling
//! and blocking modes), `stopProcess`, and `readProcessOutput` in an isolated
//! scratch workspace across four scenarios:
//! 1. Immediate exit (exit 0)
//! 2. Command that runs past the first poll (`timeout: 0`) before finishing
//! 3. Non-zero exit (both immediate startup failure and delayed non-zero exit)
//! 4. Cancellation via `stopProcess`

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

fn extract_text(result: &MCPResult) -> String {
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

async fn ensure_runtime() {
    static INIT: OnceCell<()> = OnceCell::const_new();
    INIT.get_or_init(|| async {
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

fn build_workspace_server(base_dir: &std::path::Path, session_id: &str) -> WorkspaceServer {
    let session_manager =
        SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager");
    WorkspaceServer::new(session_id.to_string(), Arc::new(session_manager))
}

#[cfg(unix)]
fn immediate_ok_cmd() -> &'static str {
    "printf 'async_immediate_ok\\n'"
}

#[cfg(windows)]
fn immediate_ok_cmd() -> &'static str {
    "Write-Output 'async_immediate_ok'"
}

#[cfg(unix)]
fn delayed_ok_cmd() -> &'static str {
    "sleep 0.6; printf 'async_after_poll\\n'"
}

#[cfg(windows)]
fn delayed_ok_cmd() -> &'static str {
    "Start-Sleep -Milliseconds 600; Write-Output 'async_after_poll'"
}

#[cfg(unix)]
fn immediate_fail_cmd() -> &'static str {
    "printf 'immediate_boom\\n' >&2; exit 42"
}

#[cfg(windows)]
fn immediate_fail_cmd() -> &'static str {
    "[Console]::Error.WriteLine('immediate_boom'); exit 42"
}

#[cfg(unix)]
fn delayed_fail_cmd() -> &'static str {
    "sleep 0.35; printf 'delayed_boom\\n' >&2; exit 7"
}

#[cfg(windows)]
fn delayed_fail_cmd() -> &'static str {
    "Start-Sleep -Milliseconds 350; [Console]::Error.WriteLine('delayed_boom'); exit 7"
}

#[cfg(unix)]
fn long_running_cmd() -> &'static str {
    "sleep 30"
}

#[cfg(windows)]
fn long_running_cmd() -> &'static str {
    "Start-Sleep -Seconds 30"
}

#[tokio::test]
async fn async_shell_command_exits_at_once_and_captures_stdout() {
    ensure_runtime().await;
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "async-shell-immediate-ok";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn = server
        .handle_spawn_process(json!({ "command": immediate_ok_cmd() }), session_id)
        .await
        .expect("spawnProcess should succeed");
    assert_eq!(spawn.is_error, Some(false));

    let data = spawn
        .structured_content
        .as_ref()
        .expect("spawn structured content");
    assert_eq!(data["mode"], "async");
    let process_id = data["process_id"].as_str().expect("process_id").to_string();

    let wait = server
        .call_tool(
            "waitForProcess",
            json!({ "processId": process_id, "timeout": 10 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("waitForProcess should succeed");
    let wait_data = wait
        .structured_content
        .as_ref()
        .expect("wait structured content");
    assert_eq!(wait_data["status"], "finished");
    assert_eq!(wait_data["exit_code"], 0);

    let read = server
        .call_tool(
            "readProcessOutput",
            json!({ "processId": process_id, "stream": "stdout", "mode": "tail", "lines": 20 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readProcessOutput should succeed");
    assert!(
        extract_text(&read).contains("async_immediate_ok"),
        "expected stdout marker, got: {}",
        extract_text(&read)
    );
}

#[tokio::test]
async fn async_shell_command_runs_past_first_poll_then_finishes() {
    ensure_runtime().await;
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "async-shell-past-first-poll";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn = server
        .handle_spawn_process(json!({ "command": delayed_ok_cmd() }), session_id)
        .await
        .expect("spawnProcess should succeed");
    assert_eq!(spawn.is_error, Some(false));

    let process_id = spawn
        .structured_content
        .as_ref()
        .and_then(|d| d.get("process_id"))
        .and_then(|v| v.as_str())
        .expect("process_id")
        .to_string();

    // First poll (timeout: 0) must observe the process still running.
    let first_poll = server
        .call_tool(
            "waitForProcess",
            json!({ "processId": process_id, "timeout": 0 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("first poll should succeed");
    let first_status = first_poll
        .structured_content
        .as_ref()
        .and_then(|d| d.get("status"))
        .and_then(|v| v.as_str())
        .expect("first poll status");
    assert!(
        matches!(first_status, "running" | "starting"),
        "expected process to still be active on first poll, got: {first_status}"
    );

    // Blocking wait settles once the command completes.
    let done = server
        .call_tool(
            "waitForProcess",
            json!({ "processId": process_id, "timeout": 10 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("waitForProcess should succeed");
    let done_data = done
        .structured_content
        .as_ref()
        .expect("done structured content");
    assert_eq!(done_data["status"], "finished");
    assert_eq!(done_data["exit_code"], 0);

    let read = server
        .call_tool(
            "readProcessOutput",
            json!({ "processId": process_id, "stream": "stdout", "mode": "tail", "lines": 20 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readProcessOutput should succeed");
    assert!(
        extract_text(&read).contains("async_after_poll"),
        "expected delayed stdout marker, got: {}",
        extract_text(&read)
    );
}

#[tokio::test]
async fn async_shell_non_zero_exit_reports_failure_for_immediate_and_delayed_commands() {
    ensure_runtime().await;
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "async-shell-non-zero";
    let server = build_workspace_server(temp_dir.path(), session_id);

    // Immediate non-zero exit (may be caught during the 100ms startup check or settle as failed).
    let immediate = server
        .handle_spawn_process(json!({ "command": immediate_fail_cmd() }), session_id)
        .await
        .expect("spawnProcess should return MCPResult");
    let imm_data = immediate
        .structured_content
        .as_ref()
        .expect("immediate structured content");
    let imm_pid = imm_data["process_id"]
        .as_str()
        .expect("immediate process_id")
        .to_string();

    if immediate.is_error == Some(true) {
        assert_eq!(imm_data["status"], "failed");
        assert_eq!(imm_data["exit_code"], 42);
        assert!(
            extract_text(&immediate).contains("immediate_boom"),
            "expected stderr in immediate failure message: {}",
            extract_text(&immediate)
        );
    } else {
        // On a heavily loaded host the 100ms startup check may finish before the shell exits.
        let wait = server
            .call_tool(
                "waitForProcess",
                json!({ "processId": imm_pid, "timeout": 10 }),
                Some(session_id.to_string()),
            )
            .await
            .expect("waitForProcess should succeed");
        let wait_data = wait
            .structured_content
            .as_ref()
            .expect("wait structured content");
        assert_eq!(wait_data["status"], "failed");
        assert_eq!(wait_data["exit_code"], 42);
    }

    // Delayed non-zero exit passes the startup check, then fails with exit code 7.
    let delayed = server
        .handle_spawn_process(json!({ "command": delayed_fail_cmd() }), session_id)
        .await
        .expect("delayed spawnProcess should succeed");
    assert_eq!(delayed.is_error, Some(false));
    let delayed_pid = delayed
        .structured_content
        .as_ref()
        .and_then(|d| d.get("process_id"))
        .and_then(|v| v.as_str())
        .expect("delayed process_id")
        .to_string();

    let delayed_wait = server
        .call_tool(
            "waitForProcess",
            json!({ "processId": delayed_pid, "timeout": 10 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("delayed waitForProcess should succeed");
    let delayed_data = delayed_wait
        .structured_content
        .as_ref()
        .expect("delayed wait structured content");
    assert_eq!(delayed_data["status"], "failed");
    assert_eq!(delayed_data["exit_code"], 7);

    let read = server
        .call_tool(
            "readProcessOutput",
            json!({ "processId": delayed_pid, "stream": "stderr", "mode": "tail", "lines": 20 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readProcessOutput should succeed");
    assert!(
        extract_text(&read).contains("delayed_boom"),
        "expected delayed stderr marker, got: {}",
        extract_text(&read)
    );
}

#[tokio::test]
async fn async_shell_cancel_via_stop_process_terminates_running_command() {
    ensure_runtime().await;
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "async-shell-cancel";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn = server
        .handle_spawn_process(json!({ "command": long_running_cmd() }), session_id)
        .await
        .expect("spawnProcess should succeed");
    assert_eq!(spawn.is_error, Some(false));

    let process_id = spawn
        .structured_content
        .as_ref()
        .and_then(|d| d.get("process_id"))
        .and_then(|v| v.as_str())
        .expect("process_id")
        .to_string();

    let poll = server
        .call_tool(
            "waitForProcess",
            json!({ "processId": process_id, "timeout": 0 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("poll should succeed");
    let poll_status = poll
        .structured_content
        .as_ref()
        .and_then(|d| d.get("status"))
        .and_then(|v| v.as_str())
        .expect("poll status");
    assert!(
        matches!(poll_status, "running" | "starting"),
        "expected running status before stop, got: {poll_status}"
    );

    let stop = server
        .call_tool(
            "stopProcess",
            json!({ "processId": process_id }),
            Some(session_id.to_string()),
        )
        .await
        .expect("stopProcess should succeed");
    assert_eq!(stop.is_error, Some(false));

    let after_stop = server
        .call_tool(
            "waitForProcess",
            json!({ "processId": process_id, "timeout": 5 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("waitForProcess after stop should succeed");
    let final_status = after_stop
        .structured_content
        .as_ref()
        .and_then(|d| d.get("status"))
        .and_then(|v| v.as_str())
        .expect("final status");
    assert_eq!(final_status, "killed");
}
