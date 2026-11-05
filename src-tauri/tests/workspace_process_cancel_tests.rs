mod common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::lifecycle::repositories::init_repositories;
use tauri_mcp_agent_lib::mcp::builtin::workspace::{WorkspaceServer, RUN_SHELL_TOOL};
use tauri_mcp_agent_lib::mcp::builtin::{ui::UiServer, BuiltinMCPServer};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tokio::time::{sleep, timeout, Duration};

#[tokio::test]
async fn cancelled_process_reports_its_status_and_later_commands_run() {
    tauri_mcp_agent_lib::lifecycle::database::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    init_repositories(&db).await;
    init_session_bus(SessionBus::new());
    init_concurrency_gate(ConcurrencyGate::new(
        DEFAULT_MAX_ACTIVE_AGENTS,
        DEFAULT_MAX_SUSPENDED_AGENTS,
        DEFAULT_MAX_ACTIVE_PROCESSES,
        DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));

    let dir = tempfile::tempdir().expect("temporary session directory");
    let manager =
        SessionManager::new_with_base_dir(dir.path().to_path_buf()).expect("session manager");
    let session = "process-cancel-regression";
    let server = Arc::new(WorkspaceServer::new(session.to_string(), Arc::new(manager)));
    #[cfg(windows)]
    let long_command = "Start-Sleep -Seconds 4";
    #[cfg(not(windows))]
    let long_command = "sleep 4";

    let running_server = server.clone();
    let running = tokio::spawn(async move {
        running_server
            .handle_run_shell(
                json!({"command": long_command, "timeout": 60}),
                session,
                RUN_SHELL_TOOL,
            )
            .await
            .expect("shell result")
    });

    timeout(Duration::from_secs(15), async {
        loop {
            let result = server
                .handle_list_processes(json!({}), session)
                .await
                .expect("process list");
            if result.structured_content.as_ref().unwrap()["running"] == 1 {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("process started");

    assert_eq!(
        server
            .kill_session_processes(session)
            .await
            .expect("cancel"),
        1
    );
    let cancelled = timeout(Duration::from_secs(15), running)
        .await
        .expect("cancel completes")
        .expect("task completes");
    assert_eq!(cancelled.is_error, Some(true));
    assert_eq!(
        cancelled.structured_content.as_ref().unwrap()["status"],
        "killed"
    );

    #[cfg(windows)]
    let next_command = "Write-Output 'after-cancel'";
    #[cfg(not(windows))]
    let next_command = "printf after-cancel";
    for command in [long_command, next_command] {
        let next = server
            .handle_run_shell(
                json!({"command": command, "timeout": 10}),
                session,
                RUN_SHELL_TOOL,
            )
            .await
            .expect("following command");
        assert_eq!(next.is_error, Some(false));
    }

    let report = UiServer::new()
        .call_tool(
            "reportResult",
            json!({"status": "success", "result": "Completed after cancellation."}),
            Some(session.to_string()),
        )
        .await
        .expect("result report after cancellation");
    assert_eq!(report.is_error, Some(false));

    assert_eq!(
        server
            .kill_session_processes(session)
            .await
            .expect("idle cancel"),
        0
    );
    let after_idle_cancel = server
        .handle_run_shell(
            json!({"command": next_command, "timeout": 10}),
            session,
            RUN_SHELL_TOOL,
        )
        .await
        .expect("command after idle cancel");
    assert_eq!(after_idle_cancel.is_error, Some(false));
}
