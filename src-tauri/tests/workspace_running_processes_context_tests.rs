//! Windows-safe coverage for Running Processes context formatting.
//!
//! Avoids constructing WorkspaceServer (full Tauri/WebView link path can fail to
//! start on Windows with STATUS_ENTRYPOINT_NOT_FOUND).

use tauri_mcp_agent_lib::agent::poll_tracker::PollTracker;
use tauri_mcp_agent_lib::mcp::builtin::workspace::context::{
    count_recently_finished_processes, format_recently_finished_processes_text,
    format_running_processes_text,
};
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
        poll_tracker: PollTracker::default(),
        first_running_poll_at: None,
    }
}

fn finished_entry(
    id: &str,
    session_id: &str,
    finished_at: chrono::DateTime<chrono::Utc>,
) -> ProcessEntry {
    let mut entry = sample_entry(id, session_id, ProcessStatus::Finished);
    entry.exit_code = Some(0);
    entry.finished_at = Some(finished_at);
    entry
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

#[tokio::test]
async fn recently_finished_lists_handoff_ids_within_window() {
    let registry = create_process_registry();
    let now = chrono::Utc::now();
    {
        let mut reg = registry.write().await;
        reg.entries.insert(
            "sync_123".to_string(),
            finished_entry("sync_123", "session-1", now),
        );
        reg.entries.insert(
            "old_done".to_string(),
            finished_entry(
                "old_done",
                "session-1",
                now - chrono::Duration::seconds(120),
            ),
        );
        reg.entries.insert(
            "other_sess".to_string(),
            finished_entry("other_sess", "session-2", now),
        );
        // Terminal without finished_at must not appear (not proven recent).
        reg.entries.insert(
            "no_ts".to_string(),
            sample_entry("no_ts", "session-1", ProcessStatus::Finished),
        );
    }

    let text = format_recently_finished_processes_text(&registry, "session-1")
        .await
        .expect("recent finished should be present");
    assert!(
        text.contains("sync_123"),
        "recent handoff id must stay visible: {text}"
    );
    assert!(
        text.contains("waitForProcess"),
        "must hint query tools: {text}"
    );
    assert!(
        !text.contains("old_done"),
        "outside window must be omitted: {text}"
    );
    assert!(
        !text.contains("other_sess"),
        "other session must not leak: {text}"
    );
    assert!(
        !text.contains("no_ts"),
        "finished without finished_at must be omitted: {text}"
    );

    assert_eq!(
        count_recently_finished_processes(&registry, "session-1").await,
        1
    );
    assert!(
        format_recently_finished_processes_text(&registry, "session-empty")
            .await
            .is_none()
    );
}
