use super::WorkspaceServer;
use crate::mcp::builtin::workspace::{
    persistent_shell, terminal_manager, InteractiveShellInputType, PendingShellExecution,
    PendingShellInputResolution, StdinDelivery,
};
use std::path::Path;
use std::sync::Arc;

#[tokio::test]
async fn kill_session_processes_marks_only_owned_processes_and_retains_output() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let session_id = "cancel-session";
    let other_session_id = "other-session";
    let session_manager = Arc::new(
        crate::session::SessionManager::new_with_base_dir(temp_dir.path().to_path_buf())
            .expect("session manager"),
    );
    let server = WorkspaceServer::new(session_id.to_string(), session_manager);

    let retained_output = temp_dir.path().join("cancel-session").join("stdout.log");
    std::fs::create_dir_all(retained_output.parent().expect("output parent"))
        .expect("output directory");
    std::fs::write(&retained_output, "partial output").expect("output file");

    let process_entry =
        |id: &str, owner: &str, stdout_path: &Path| terminal_manager::ProcessEntry {
            id: id.to_string(),
            name: None,
            session_id: owner.to_string(),
            command: "long-running-command".to_string(),
            status: terminal_manager::ProcessStatus::Running,
            pid: None,
            exit_code: None,
            started_at: chrono::Utc::now(),
            finished_at: None,
            stdout_path: stdout_path.to_string_lossy().to_string(),
            stderr_path: stdout_path.to_string_lossy().to_string(),
            stdout_size: 0,
            stderr_size: 0,
            last_poll_at: None,
            poll_count: 0,
            poll_tracker: crate::agent::poll_tracker::PollTracker::default(),
            first_running_poll_at: None,
        };

    let other_output = temp_dir.path().join("other").join("stdout.log");
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    {
        let mut registry = server.process_registry.write().await;
        registry.entries.insert(
            "owned-process".to_string(),
            process_entry("owned-process", session_id, &retained_output),
        );
        let mut starting_process = process_entry("owned-starting", session_id, &retained_output);
        starting_process.status = terminal_manager::ProcessStatus::Starting;
        registry
            .entries
            .insert("owned-starting".to_string(), starting_process);
        registry.entries.insert(
            "foreign-process".to_string(),
            process_entry("foreign-process", other_session_id, &other_output),
        );
    }
    server.pending_executions.insert(PendingShellExecution {
        execution_id: "interactive-1".to_string(),
        session_id: session_id.to_string(),
        executable_command: "read-host".to_string(),
        display_command: "read-host".to_string(),
        run_mode: "sync".to_string(),
        timeout: 30,
        created_at: chrono::Utc::now(),
        prompt: "Input".to_string(),
        input_type: InteractiveShellInputType::Text,
        stdin_delivery: StdinDelivery::Host,
        response_tx: Some(response_tx),
    });

    assert_eq!(
        server
            .kill_session_processes(session_id)
            .await
            .expect("cancel resources"),
        2
    );

    let registry = server.process_registry.read().await;
    let owned = registry
        .entries
        .get("owned-process")
        .expect("owned process");
    assert_eq!(owned.status, terminal_manager::ProcessStatus::Killed);
    assert!(owned.finished_at.is_some());
    assert_eq!(
        registry
            .entries
            .get("owned-starting")
            .expect("starting process")
            .status,
        terminal_manager::ProcessStatus::Killed
    );
    assert_eq!(
        registry
            .entries
            .get("foreign-process")
            .expect("foreign process")
            .status,
        terminal_manager::ProcessStatus::Running
    );
    drop(registry);

    assert!(retained_output.exists());
    assert!(matches!(
        response_rx.await.expect("pending response"),
        PendingShellInputResolution::Cancelled
    ));
}

#[tokio::test]
async fn force_kills_persistent_shell_without_leaving_manager_entry() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let workspace_path = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).expect("workspace directory");
    let manager = persistent_shell::PersistentShellManager::new();

    manager
        .get_or_create_shell("persistent-cancel".to_string(), workspace_path)
        .await
        .expect("persistent shell");
    assert_eq!(manager.shell_count().await, 1);

    assert!(manager
        .force_kill_shell("persistent-cancel")
        .await
        .expect("force kill shell"));
    assert_eq!(manager.shell_count().await, 0);
}

#[test]
fn test_extract_teamwork_alias_relative_path() {
    assert_eq!(
        WorkspaceServer::extract_teamwork_alias_relative_path("@teamwork"),
        Some(".")
    );
    assert_eq!(
        WorkspaceServer::extract_teamwork_alias_relative_path("@teamwork/coordination/KANBAN.md"),
        Some("coordination/KANBAN.md")
    );
    assert_eq!(
        WorkspaceServer::extract_teamwork_alias_relative_path("@teamwork\\coordination\\KANBAN.md"),
        Some("coordination\\KANBAN.md")
    );

    assert_eq!(
        WorkspaceServer::extract_teamwork_alias_relative_path(".libragent/teamwork"),
        Some(".")
    );
    assert_eq!(
        WorkspaceServer::extract_teamwork_alias_relative_path(
            ".libragent/teamwork/coordination/KANBAN.md"
        ),
        Some("coordination/KANBAN.md")
    );
    assert_eq!(
        WorkspaceServer::extract_teamwork_alias_relative_path(
            ".libragent\\teamwork\\coordination\\KANBAN.md"
        ),
        Some("coordination\\KANBAN.md")
    );

    assert_eq!(
        WorkspaceServer::extract_teamwork_alias_relative_path("src/main.rs"),
        None
    );
    assert_eq!(
        WorkspaceServer::extract_teamwork_alias_relative_path("docs/README.md"),
        None
    );
}

#[test]
fn test_extract_absolute_teamwork_relative_path_for_new_files() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let teamwork_root = temp_dir.path().join("teamwork-artifacts").join("session-1");
    std::fs::create_dir_all(&teamwork_root).expect("create teamwork root");

    let new_file = teamwork_root.join("coordination").join("KANBAN.md");
    let relative =
        WorkspaceServer::extract_absolute_teamwork_relative_path(&new_file, &teamwork_root)
            .expect("new absolute path under teamwork root must map to relative");
    assert_eq!(relative, "coordination/KANBAN.md");

    let outside = temp_dir.path().join("outside.md");
    assert!(
        WorkspaceServer::extract_absolute_teamwork_relative_path(&outside, &teamwork_root)
            .is_none()
    );

    let traversal = teamwork_root.join("..").join("outside.md");
    assert!(
        WorkspaceServer::extract_absolute_teamwork_relative_path(&traversal, &teamwork_root)
            .is_none(),
        "parent-dir components after the teamwork root must be rejected"
    );
}
