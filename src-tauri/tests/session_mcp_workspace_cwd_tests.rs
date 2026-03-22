use std::collections::HashMap;

use tauri_mcp_agent_lib::mcp::session_isolation_config::SessionIsolationConfig;
use tauri_mcp_agent_lib::mcp::types::{MCPServerConfig, TransportConfig};
use tauri_mcp_agent_lib::mcp::SessionMCPManager;

fn create_manager(
    workspace_dir: std::path::PathBuf,
    cwd_capture_file: std::path::PathBuf,
) -> SessionMCPManager {
    let mock_server_path = std::env::current_dir()
        .expect("repo cwd")
        .join("tests")
        .join("mock_server.py");

    let mut env = HashMap::new();
    env.insert(
        "MOCK_SERVER_CWD_FILE".to_string(),
        cwd_capture_file.to_string_lossy().to_string(),
    );

    let mut configs = HashMap::new();
    configs.insert(
        "test-server".to_string(),
        MCPServerConfig {
            name: Some("test-server".to_string()),
            transport: TransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![mock_server_path.to_string_lossy().to_string()],
                env,
            },
            authentication: None,
            metadata: None,
        },
    );

    SessionMCPManager::new(
        "test-session".to_string(),
        configs,
        SessionIsolationConfig {
            idle_timeout_minutes: 5,
            cleanup_interval_minutes: 5,
            process_startup_timeout_seconds: 30,
            max_restart_attempts: 0,
            http_connection_pool_size: 10,
        },
        workspace_dir,
    )
}

#[cfg(unix)]
#[tokio::test]
async fn test_session_mcp_servers_start_in_workspace() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("Skipping integration test: python3 not available");
        return;
    }

    let workspace_dir = std::env::temp_dir().join(format!(
        "libragent-session-workspace-{}",
        uuid::Uuid::new_v4()
    ));
    let cwd_capture_file = std::env::temp_dir().join(format!(
        "libragent-session-workspace-cwd-{}.txt",
        uuid::Uuid::new_v4()
    ));

    if workspace_dir.exists() {
        std::fs::remove_dir_all(&workspace_dir).unwrap();
    }
    if cwd_capture_file.exists() {
        std::fs::remove_file(&cwd_capture_file).unwrap();
    }

    let manager = create_manager(workspace_dir.clone(), cwd_capture_file.clone());
    let tools = manager
        .list_tools("test-server")
        .await
        .expect("mock server should start and answer tools/list");

    assert!(tools.is_empty(), "mock server should expose no tools");
    assert!(
        workspace_dir.exists(),
        "session workspace should be created before spawning stdio MCP servers"
    );
    assert!(
        cwd_capture_file.exists(),
        "mock server should capture its current working directory"
    );

    let captured_cwd = std::fs::read_to_string(&cwd_capture_file).unwrap();
    assert_eq!(
        std::path::Path::new(captured_cwd.trim()),
        workspace_dir.as_path(),
        "session MCP server should start from the session workspace"
    );

    let _ = std::fs::remove_file(cwd_capture_file);
    let _ = std::fs::remove_dir_all(workspace_dir);
}
