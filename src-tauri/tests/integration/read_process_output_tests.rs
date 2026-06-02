use crate::common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::lifecycle::repositories::init_repositories;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
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
fn dual_stream_command() -> &'static str {
    "printf 'stdout line 1\\nstdout line 2\\n'; printf 'stderr line 1\\nstderr line 2\\n' >&2"
}

#[cfg(windows)]
fn dual_stream_command() -> &'static str {
    "Write-Output 'stdout line 1'; Write-Output 'stdout line 2'; [Console]::Error.WriteLine('stderr line 1'); [Console]::Error.WriteLine('stderr line 2')"
}

async fn wait_for_terminal_state(server: &WorkspaceServer, process_id: &str, session_id: &str) {
    let wait_result = server
        .call_tool(
            "waitForProcess",
            json!({ "processId": process_id, "timeout": 10 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("waitForProcess should succeed");

    let structured = wait_result
        .structured_content
        .as_ref()
        .expect("waitForProcess structured content expected");
    let status = structured["status"]
        .as_str()
        .expect("waitForProcess should return status");

    assert!(
        matches!(status, "finished" | "failed" | "killed"),
        "process did not reach a terminal state: {status}"
    );
}

#[tokio::test]
async fn read_process_output_both_returns_both_sections_and_structured_outputs() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-process-output-both";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn_result = server
        .handle_spawn_process(json!({ "command": dual_stream_command() }), session_id)
        .await
        .expect("spawnProcess should succeed");
    let process_id = spawn_result
        .structured_content
        .as_ref()
        .and_then(|data| data.get("process_id"))
        .and_then(|value| value.as_str())
        .expect("spawnProcess should return process_id")
        .to_string();

    wait_for_terminal_state(&server, &process_id, session_id).await;

    let read_result = server
        .call_tool(
            "readProcessOutput",
            json!({
                "processId": process_id,
                "stream": "both",
                "mode": "tail",
                "lines": 10
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readProcessOutput should succeed");

    assert_eq!(read_result.is_error, Some(false));

    let text = extract_text_content(&read_result);
    assert!(text.contains("[STDOUT]"), "stdout section missing: {text}");
    assert!(text.contains("[STDERR]"), "stderr section missing: {text}");
    assert!(
        text.contains("stdout line 1"),
        "stdout content missing: {text}"
    );
    assert!(
        text.contains("stderr line 1"),
        "stderr content missing: {text}"
    );
    assert!(
        text.contains("Internal output files (absolute paths, not workspace-relative):"),
        "output paths section should clearly explain path scope: {text}"
    );
    assert!(
        text.contains("not workspace-relative"),
        "readProcessOutput should warn that these paths cannot be fed back into workspace file tools: {text}"
    );

    let structured = read_result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(structured["stream"], json!("both"));
    assert_eq!(structured["mode"], json!("tail"));
    assert_eq!(structured["status"], json!("finished"));
    assert_eq!(structured["is_process_running"], json!(false));
    assert_eq!(structured["lines_requested"], json!(10));

    let stdout = &structured["outputs"]["stdout"];
    let stderr = &structured["outputs"]["stderr"];
    let stdout_path = structured["output_paths"]["stdout"]
        .as_str()
        .expect("stdout output path should be present");
    let stderr_path = structured["output_paths"]["stderr"]
        .as_str()
        .expect("stderr output path should be present");
    assert!(
        stdout["content"].to_string().contains("stdout line 1"),
        "stdout structured content missing expected line: {}",
        stdout["content"]
    );
    assert!(
        stderr["content"].to_string().contains("stderr line 1"),
        "stderr structured content missing expected line: {}",
        stderr["content"]
    );
    assert_eq!(stdout["lines_returned"], json!(2));
    assert_eq!(stderr["lines_returned"], json!(2));
    assert!(
        stdout["total_size_bytes"].as_u64().unwrap_or(0) > 0,
        "stdout total_size_bytes should be populated"
    );
    assert!(
        stderr["total_size_bytes"].as_u64().unwrap_or(0) > 0,
        "stderr total_size_bytes should be populated"
    );
    assert!(
        text.contains(stdout_path),
        "stdout output path missing from text: {text}"
    );
    assert!(
        text.contains(stderr_path),
        "stderr output path missing from text: {text}"
    );
    assert!(
        std::path::Path::new(stdout_path).exists(),
        "stdout output path should exist: {stdout_path}"
    );
    assert!(
        std::path::Path::new(stderr_path).exists(),
        "stderr output path should exist: {stderr_path}"
    );
}

#[tokio::test]
async fn read_process_output_stdout_returns_single_output_path() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-process-output-stdout-only";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn_result = server
        .handle_spawn_process(json!({ "command": dual_stream_command() }), session_id)
        .await
        .expect("spawnProcess should succeed");
    let process_id = spawn_result
        .structured_content
        .as_ref()
        .and_then(|data| data.get("process_id"))
        .and_then(|value| value.as_str())
        .expect("spawnProcess should return process_id")
        .to_string();

    wait_for_terminal_state(&server, &process_id, session_id).await;

    let result = server
        .call_tool(
            "readProcessOutput",
            json!({
                "processId": process_id,
                "stream": "stdout",
                "mode": "tail",
                "lines": 10
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readProcessOutput should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(text.contains("[STDOUT]"), "stdout section missing: {text}");
    assert!(
        !text.contains("[STDERR]"),
        "stderr section should be absent: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert!(structured["output_paths"]["stdout"].is_string());
    assert!(structured["output_paths"].get("stderr").is_none());
    assert!(structured["outputs"]["stdout"].is_object());
    assert!(structured["outputs"].get("stderr").is_none());
}

#[tokio::test]
async fn spawn_and_list_processes_surface_optional_name_labels() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "process-name-labels";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn_result = server
        .handle_spawn_process(
            json!({
                "command": dual_stream_command(),
                "name": "demo-process"
            }),
            session_id,
        )
        .await
        .expect("spawnProcess should succeed");

    let spawn_text = extract_text_content(&spawn_result);
    assert!(
        spawn_text.contains("• Name: demo-process"),
        "spawn response should expose process name label: {spawn_text}"
    );

    let structured = spawn_result
        .structured_content
        .as_ref()
        .expect("spawn structured content expected");
    let process_id = structured["process_id"]
        .as_str()
        .expect("spawnProcess should return process_id")
        .to_string();
    assert_eq!(structured["name"], json!("demo-process"));

    let list_result = server
        .handle_list_processes(json!({}), session_id)
        .await
        .expect("listProcesses should succeed");

    let list_text = extract_text_content(&list_result);
    assert!(
        list_text.contains("Name: demo-process"),
        "listProcesses should show the optional process name: {list_text}"
    );

    let processes = list_result
        .structured_content
        .as_ref()
        .and_then(|value| value.get("processes"))
        .and_then(|value| value.as_array())
        .expect("process list expected");
    assert!(
        processes.iter().any(|item| {
            item["process_id"] == json!(process_id) && item["name"] == json!("demo-process")
        }),
        "structured process list should include process name"
    );
}

#[tokio::test]
async fn list_processes_prefers_read_output_for_finished_processes() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "list-processes-finished-hints";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn_result = server
        .handle_spawn_process(json!({ "command": dual_stream_command() }), session_id)
        .await
        .expect("spawnProcess should succeed");
    let process_id = spawn_result
        .structured_content
        .as_ref()
        .and_then(|data| data.get("process_id"))
        .and_then(|value| value.as_str())
        .expect("spawnProcess should return process_id")
        .to_string();

    wait_for_terminal_state(&server, &process_id, session_id).await;

    let list_result = server
        .handle_list_processes(json!({}), session_id)
        .await
        .expect("listProcesses should succeed");

    let text = extract_text_content(&list_result);
    assert!(
        text.contains(&format!(
            "Use readProcessOutput('{}', 'both') to inspect stdout and stderr",
            process_id
        )),
        "finished processes should point to readProcessOutput first: {text}"
    );
    assert!(
        !text.contains(&format!(
            "Use waitForProcess('{}', 0) to check status",
            process_id
        )),
        "finished processes should not suggest polling again as the primary next step: {text}"
    );
    assert!(
        !text.contains("Use stopProcess"),
        "finished processes should not suggest stopProcess: {text}"
    );
}

#[tokio::test]
async fn read_process_output_avoids_stop_hint_after_process_has_finished() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-process-output-finished-hints";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let spawn_result = server
        .handle_spawn_process(json!({ "command": dual_stream_command() }), session_id)
        .await
        .expect("spawnProcess should succeed");
    let process_id = spawn_result
        .structured_content
        .as_ref()
        .and_then(|data| data.get("process_id"))
        .and_then(|value| value.as_str())
        .expect("spawnProcess should return process_id")
        .to_string();

    wait_for_terminal_state(&server, &process_id, session_id).await;

    let result = server
        .call_tool(
            "readProcessOutput",
            json!({
                "processId": process_id,
                "stream": "both",
                "mode": "tail",
                "lines": 10
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readProcessOutput should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("Analyze the captured output to verify command success"),
        "finished processes should suggest analyzing the finished output: {text}"
    );
    assert!(
        !text.contains("Use stopProcess"),
        "finished processes should not suggest stopProcess: {text}"
    );
    assert!(
        !text.contains("waitForProcess"),
        "finished processes should not suggest waiting again: {text}"
    );
}
