use super::*;

#[cfg(unix)]
const LONG_COMMAND: &str = "echo retained-start; sleep 30";
#[cfg(windows)]
const LONG_COMMAND: &str = "Write-Output retained-start; Start-Sleep -Seconds 30";

#[cfg(unix)]
const PERSISTENT_SHELL_TOOL: &str = "workspace__runInPersistentShell";
#[cfg(windows)]
const PERSISTENT_SHELL_TOOL: &str = "workspace__runInPersistentPowerShell";
#[cfg(unix)]
const SET_SHELL_MARK: &str = "export RETAINED_MARK=kept-7731";
#[cfg(windows)]
const SET_SHELL_MARK: &str = "$env:RETAINED_MARK = 'kept-7731'";
#[cfg(unix)]
const READ_SHELL_STATE: &str = "echo \"mark=$RETAINED_MARK len=${#RETAIN_TOKEN}\"";
#[cfg(windows)]
const READ_SHELL_STATE: &str =
    "$t = \"$env:RETAIN_TOKEN\"; Write-Output \"mark=$env:RETAINED_MARK len=$($t.Length)\"";

/// Releases the sessions' retained state even when an assertion fails, so one failed
/// test does not leave entries in the process-wide store for later tests.
struct ReleaseOnDrop(&'static [&'static str]);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        for session_id in self.0 {
            crate::mcp::builtin::workspace::release_session_workspace_state(session_id);
        }
    }
}

struct RetentionHarness {
    _harness: TestHarness,
    _workspace_root: tempfile::TempDir,
    manager: Arc<MCPServiceProxyManager>,
}

/// The shared harness, but with session workspaces under a temp dir: these tests
/// start real processes.
async fn retention_harness() -> RetentionHarness {
    let harness = create_test_harness().await;
    let workspace_root = tempfile::tempdir().expect("temp dir");
    let session_manager = Arc::new(
        crate::session::SessionManager::new_with_base_dir(workspace_root.path().to_path_buf())
            .expect("session manager"),
    );
    let manager = Arc::new(MCPServiceProxyManager::new(
        harness.manager.db.clone(),
        session_manager,
    ));
    RetentionHarness {
        _harness: harness,
        _workspace_root: workspace_root,
        manager,
    }
}

fn response_text(response: &crate::mcp::MCPResponse) -> String {
    assert!(response.error.is_none(), "tool error: {:?}", response.error);
    serde_json::to_string(response).expect("serialize response")
}

fn find_process_id(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => map
            .get("process_id")
            .and_then(|id| id.as_str())
            .map(str::to_string)
            .or_else(|| map.values().find_map(find_process_id)),
        serde_json::Value::Array(items) => items.iter().find_map(find_process_id),
        _ => None,
    }
}

async fn spawn_long_process(manager: &MCPServiceProxyManager, session_id: &str) -> String {
    let response = manager
        .call_tool(
            session_id,
            "workspace__spawnProcess",
            json!({ "command": LONG_COMMAND }),
        )
        .await
        .expect("spawnProcess");
    response_text(&response);
    let value = serde_json::to_value(&response).expect("response json");
    find_process_id(&value).expect("spawnProcess returns a process id")
}

async fn list_processes(manager: &MCPServiceProxyManager, session_id: &str) -> String {
    let response = manager
        .call_tool(session_id, "workspace__listProcesses", json!({}))
        .await
        .expect("listProcesses");
    response_text(&response)
}

async fn stop_process(manager: &MCPServiceProxyManager, session_id: &str, process_id: &str) {
    let response = manager
        .call_tool(
            session_id,
            "workspace__stopProcess",
            json!({ "processId": process_id }),
        )
        .await
        .expect("stopProcess");
    response_text(&response);
}

#[tokio::test]
async fn reload_and_config_recreation_keep_a_running_process_listed_readable_and_stoppable() {
    let harness = retention_harness().await;
    let manager = &harness.manager;
    let session_id = "process-retention-reload";
    let other_session = "process-retention-other";
    let _release = ReleaseOnDrop(&["process-retention-reload", "process-retention-other"]);
    insert_test_session(manager, session_id).await;
    insert_test_session(manager, other_session).await;

    manager.ensure_builtin_proxy(session_id).await.unwrap();
    manager.ensure_builtin_proxy(other_session).await.unwrap();
    let process_id = spawn_long_process(manager, session_id).await;
    let other_process_id = spawn_long_process(manager, other_session).await;

    // Reload: the proxy and its WorkspaceServer are destroyed and created again, twice.
    for _ in 0..2 {
        manager.destroy_proxy(session_id).await;
        manager.ensure_builtin_proxy(session_id).await.unwrap();
    }
    let listed = list_processes(manager, session_id).await;
    assert!(listed.contains(&process_id), "{listed}");
    assert!(!listed.contains(&other_process_id), "{listed}");

    // Config-driven recreation replaces the proxy through create_proxy.
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["workspace".to_string(), "playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();

    let output = manager
        .call_tool(
            session_id,
            "workspace__readProcessOutput",
            json!({ "processId": process_id, "stream": "stdout" }),
        )
        .await
        .expect("readProcessOutput");
    let output = response_text(&output);
    assert!(output.contains("retained-start"), "{output}");

    stop_process(manager, session_id, &process_id).await;
    let after_stop = list_processes(manager, session_id).await;
    assert!(after_stop.contains(&process_id), "{after_stop}");
    assert!(
        !list_processes(manager, other_session)
            .await
            .contains(&process_id),
        "ownership stays with the session that spawned the process"
    );
    let other_listed = list_processes(manager, other_session).await;
    assert!(other_listed.contains(&other_process_id), "{other_listed}");

    stop_process(manager, other_session, &other_process_id).await;
}

#[tokio::test]
async fn releasing_the_session_state_starts_the_next_server_empty() {
    let harness = retention_harness().await;
    let manager = &harness.manager;
    let session_id = "process-retention-release";
    let _release = ReleaseOnDrop(&["process-retention-release"]);
    insert_test_session(manager, session_id).await;

    manager.ensure_builtin_proxy(session_id).await.unwrap();
    let process_id = spawn_long_process(manager, session_id).await;
    stop_process(manager, session_id, &process_id).await;

    manager.destroy_proxy(session_id).await;
    assert!(crate::mcp::builtin::workspace::release_session_workspace_state(session_id));
    manager.ensure_builtin_proxy(session_id).await.unwrap();

    let listed = list_processes(manager, session_id).await;
    assert!(!listed.contains(&process_id), "{listed}");
}

async fn run_in_persistent_shell(
    manager: &MCPServiceProxyManager,
    session_id: &str,
    command: &str,
) -> String {
    let response = manager
        .call_tool(
            session_id,
            PERSISTENT_SHELL_TOOL,
            json!({ "command": command }),
        )
        .await
        .expect("persistent shell");
    response_text(&response)
}

#[tokio::test]
async fn reload_keeps_the_persistent_shell_and_stored_values_until_release() {
    use crate::mcp::builtin::workspace::workspace_server::retained::retained_state;

    let harness = retention_harness().await;
    let manager = &harness.manager;
    let session_id = "process-retention-shell";
    let _release = ReleaseOnDrop(&["process-retention-shell"]);
    insert_test_session(manager, session_id).await;
    let stored = "retained-secret-5521";

    manager.ensure_builtin_proxy(session_id).await.unwrap();
    retained_state(session_id)
        .shell_manager
        .store_secret(session_id, "RETAIN_TOKEN", stored.to_string())
        .await
        .expect("store secret");
    run_in_persistent_shell(manager, session_id, SET_SHELL_MARK).await;

    for _ in 0..2 {
        manager.destroy_proxy(session_id).await;
        manager.ensure_builtin_proxy(session_id).await.unwrap();
    }
    let kept = run_in_persistent_shell(manager, session_id, READ_SHELL_STATE).await;
    assert!(
        kept.contains("mark=kept-7731"),
        "same shell after reload: {kept}"
    );
    assert!(
        kept.contains(&format!("len={}", stored.len())),
        "stored value still set: {kept}"
    );
    assert_eq!(
        retained_state(session_id)
            .shell_manager
            .session_secrets(session_id)
            .await
            .get("RETAIN_TOKEN")
            .map(String::as_str),
        Some(stored)
    );

    manager.destroy_proxy(session_id).await;
    assert!(crate::mcp::builtin::workspace::release_session_workspace_state(session_id));
    manager.ensure_builtin_proxy(session_id).await.unwrap();

    let fresh = run_in_persistent_shell(manager, session_id, READ_SHELL_STATE).await;
    assert!(
        !fresh.contains("kept-7731"),
        "new shell after release: {fresh}"
    );
    assert!(fresh.contains("len=0"), "stored value gone: {fresh}");
    assert!(retained_state(session_id)
        .shell_manager
        .session_secrets(session_id)
        .await
        .is_empty());
}

#[tokio::test]
async fn terminate_after_reload_refuses_a_late_persistent_shell() {
    use crate::mcp::builtin::workspace::workspace_server::retained::retained_state;

    let harness = retention_harness().await;
    let manager = &harness.manager;
    let session_id = "process-retention-terminate";
    let _release = ReleaseOnDrop(&["process-retention-terminate"]);
    insert_test_session(manager, session_id).await;

    manager.ensure_builtin_proxy(session_id).await.unwrap();
    run_in_persistent_shell(manager, session_id, SET_SHELL_MARK).await;
    for _ in 0..2 {
        manager.destroy_proxy(session_id).await;
        manager.ensure_builtin_proxy(session_id).await.unwrap();
    }

    manager.terminate_session_resources(session_id).await;

    // A late dispatch on any server of the session reaches this shared manager; the
    // terminate marked through the current server must fence it.
    let shell_manager = retained_state(session_id).shell_manager;
    assert!(shell_manager.is_session_terminated());
    let workspace = tempfile::tempdir().expect("temp dir");
    let late = shell_manager
        .get_or_create_shell(session_id.to_string(), workspace.path().to_path_buf())
        .await;
    assert!(late.is_err(), "late shell after terminate must be refused");
    assert_eq!(shell_manager.shell_count().await, 0);
}
