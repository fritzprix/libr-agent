//! Windows-safe coverage for Running Processes context formatting.
//!
//! Avoids constructing WorkspaceServer (full Tauri/WebView link path can fail to
//! start on Windows with STATUS_ENTRYPOINT_NOT_FOUND).

use tauri_mcp_agent_lib::mcp::builtin::workspace::context::format_running_processes_text;
use tauri_mcp_agent_lib::mcp::builtin::workspace::terminal_manager::{
    create_process_registry, ProcessEntry, ProcessStatus,
};

fn sample_entry(id: &str, session_id: &str, status: ProcessStatus) -> ProcessEntry {
    ProcessEntry {
        id: id.to_string(),
        name: None,
        session_id: session_id.to_string(),
        command: format!("echo {id}"),
        status,
        pid: Some(1),
        exit_code: None,
        started_at: chrono::Utc::now(),
        finished_at: None,
        stdout_path: "/tmp/out".to_string(),
        stderr_path: "/tmp/err".to_string(),
        stdout_size: 0,
        stderr_size: 0,
        last_poll_at: None,
        poll_count: 0,
        consecutive_running_polls: 0,
        first_running_poll_at: None,
    }
}

#[tokio::test]
async fn format_running_processes_lists_only_active_for_session() {
    let registry = create_process_registry();
    {
        let mut reg = registry.write().await;
        reg.entries.insert(
            "proc-a".to_string(),
            sample_entry("proc-a", "session-1", ProcessStatus::Running),
        );
        reg.entries.insert(
            "proc-b".to_string(),
            sample_entry("proc-b", "session-1", ProcessStatus::Finished),
        );
        reg.entries.insert(
            "proc-c".to_string(),
            sample_entry("proc-c", "session-2", ProcessStatus::Running),
        );
    }

    let text = format_running_processes_text(&registry, "session-1").await;
    assert!(
        text.contains("proc-a"),
        "active process must be listed: {text}"
    );
    assert!(
        !text.contains("proc-b"),
        "finished process must not be listed as running: {text}"
    );
    assert!(
        !text.contains("proc-c"),
        "other-session process must not leak: {text}"
    );
    assert!(text.starts_with('1'), "running count should be 1: {text}");
}

#[tokio::test]
async fn format_running_processes_reports_none_when_idle() {
    let registry = create_process_registry();
    {
        let mut reg = registry.write().await;
        reg.entries.insert(
            "proc-done".to_string(),
            sample_entry("proc-done", "session-1", ProcessStatus::Finished),
        );
    }

    let text = format_running_processes_text(&registry, "session-1").await;
    assert_eq!(text, "None");
}
