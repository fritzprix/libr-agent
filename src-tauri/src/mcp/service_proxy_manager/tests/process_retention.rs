use super::*;

#[cfg(unix)]
const LONG_COMMAND: &str = "echo retained-start; sleep 30";
#[cfg(windows)]
const LONG_COMMAND: &str = "Write-Output retained-start; Start-Sleep -Seconds 30";

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
    crate::mcp::builtin::workspace::release_session_workspace_state(session_id);
    crate::mcp::builtin::workspace::release_session_workspace_state(other_session);
}

#[tokio::test]
async fn releasing_the_session_state_starts_the_next_server_empty() {
    let harness = retention_harness().await;
    let manager = &harness.manager;
    let session_id = "process-retention-release";
    insert_test_session(manager, session_id).await;

    manager.ensure_builtin_proxy(session_id).await.unwrap();
    let process_id = spawn_long_process(manager, session_id).await;
    stop_process(manager, session_id, &process_id).await;

    manager.destroy_proxy(session_id).await;
    assert!(crate::mcp::builtin::workspace::release_session_workspace_state(session_id));
    manager.ensure_builtin_proxy(session_id).await.unwrap();

    let listed = list_processes(manager, session_id).await;
    assert!(!listed.contains(&process_id), "{listed}");
    crate::mcp::builtin::workspace::release_session_workspace_state(session_id);
}
