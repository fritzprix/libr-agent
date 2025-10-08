use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::session::SessionManager;

// Helper to create a WorkspaceServer instance for testing
fn setup_workspace_server() -> WorkspaceServer {
    let session_manager =
        Arc::new(SessionManager::new().expect("Failed to create test session manager"));
    WorkspaceServer::new(session_manager)
}

#[tokio::test]
async fn test_integration_open_terminal_and_execute_shell_sync() {
    let server = setup_workspace_server();

    // 1. Open a new terminal
    let open_args = json!({});
    let open_response = server.call_tool("open_new_terminal", open_args).await;
    assert!(open_response.error.is_none());
    let result_val = open_response.result.expect("result should exist");
    let terminal_id = result_val["terminal_id"]
        .as_str()
        .expect("Result should contain a terminal_id string")
        .to_string();
    assert!(!terminal_id.is_empty());

    // 2. Execute a simple command synchronously
    let exec_args = json!({
        "terminal_id": terminal_id,
        "command": "echo 'sync test successful'",
        "async": false
    });
    let exec_response = server.call_tool("execute_shell", exec_args).await;

    assert!(
        exec_response.error.is_none(),
        "Shell execution should be successful. Error: {:?}",
        exec_response.error
    );

    let result_value = exec_response.result.expect("result should exist");
    let result_text = result_value
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.get(0))
        .and_then(|i| i.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    assert!(
        result_text.contains("sync test successful"),
        "Sync output should contain the echoed string."
    );

    // 3. Close the terminal
    let close_args = json!({ "terminal_id": terminal_id });
    server.call_tool("close_terminal", close_args).await;
}

#[tokio::test]
async fn test_integration_execute_shell_async_and_read_output() {
    let server = setup_workspace_server();

    // 1. Open a new terminal
    let open_args = json!({});
    let open_response = server.call_tool("open_new_terminal", open_args).await;
    assert!(open_response.error.is_none());
    let result_val = open_response.result.expect("result should exist");
    let terminal_id = result_val["terminal_id"]
        .as_str()
        .expect("Result should contain a terminal_id string")
        .to_string();

    // 2. Execute a command asynchronously
    let exec_args = json!({
        "terminal_id": terminal_id,
        "command": "sleep 0.2; echo 'async test successful'",
        "async": true
    });
    let exec_response = server.call_tool("execute_shell", exec_args).await;
    assert!(
        exec_response.error.is_none(),
        "Async shell execution should return success immediately. Error: {:?}",
        exec_response.error
    );

    // 3. Poll for the output
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let read_args = json!({ "terminal_id": terminal_id });
    let read_response = server.call_tool("read_terminal_output", read_args).await;
    assert!(read_response.error.is_none());

    let result_value = read_response.result.expect("result should exist");
    let read_result_str = result_value
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.get(0))
        .and_then(|i| i.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let read_result: serde_json::Value =
        serde_json::from_str(read_result_str).expect("Should be valid JSON");

    let output_text = read_result["output"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["line"].as_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        output_text.contains("async test successful"),
        "Async output should contain the echoed string."
    );

    // 4. Close the terminal
    let close_args = json!({ "terminal_id": terminal_id });
    server.call_tool("close_terminal", close_args).await;
}

#[tokio::test]
async fn test_integration_env_persistence_in_terminal() {
    let server = setup_workspace_server();
    let open_response = server.call_tool("open_new_terminal", json!({})).await;
    assert!(open_response.error.is_none());
    let result_val = open_response.result.expect("result should exist");
    let terminal_id = result_val["terminal_id"]
        .as_str()
        .expect("Result should contain a terminal_id string")
        .to_string();

    // Execute a command that sets and then uses an environment variable in one go.
    let chained_command_args = json!({
        "terminal_id": terminal_id,
        "command": "export MY_TEST_VAR='hello_from_env' && echo $MY_TEST_VAR",
        "async": false,
    });
    let exec_response = server
        .call_tool("execute_shell", chained_command_args)
        .await;

    assert!(
        exec_response.error.is_none(),
        "Chained command execution should be successful. Error: {:?}",
        exec_response.error
    );

    let result_value = exec_response.result.expect("result should exist");
    let result_text = result_value
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.get(0))
        .and_then(|i| i.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    assert!(
        result_text.contains("hello_from_env"),
        "Output should contain the value of the environment variable."
    );

    server
        .call_tool("close_terminal", json!({ "terminal_id": terminal_id }))
        .await;
}

#[tokio::test]
async fn test_negative_execute_shell_without_terminal_id() {
    let server = setup_workspace_server();
    let exec_args = json!({
        "command": "echo 'this should fail'",
    });

    let exec_response = server.call_tool("execute_shell", exec_args).await;

    assert!(
        exec_response.error.is_some(),
        "Execution should fail without a terminal_id."
    );
    assert!(exec_response
        .error
        .expect("error should exist")
        .message
        .contains("Missing required parameter: terminal_id"));
}
