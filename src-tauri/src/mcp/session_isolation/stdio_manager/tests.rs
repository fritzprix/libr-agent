use super::SessionMCPManager;
use crate::mcp::session_isolation::error::SessionMCPError;
use crate::mcp::session_isolation_config::SessionIsolationConfig;
use crate::mcp::types::{MCPServerConfig, TransportConfig};
use std::collections::HashMap;
use std::time::Duration;

/// Helper to create a test manager with a simple echo server config
fn create_test_manager() -> SessionMCPManager {
    let mut configs = HashMap::new();
    let mut env_vars = HashMap::new();
    env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

    // Use a simple command that exists on all platforms
    #[cfg(windows)]
    let command = "cmd.exe";
    #[cfg(not(windows))]
    let command = "echo";

    configs.insert(
        "test-server".to_string(),
        MCPServerConfig {
            name: Some("test-server".to_string()),
            transport: TransportConfig::Stdio {
                command: command.to_string(),
                args: vec![],
                env: env_vars,
            },
            authentication: None,
            metadata: None,
        },
    );

    let config = SessionIsolationConfig {
        idle_timeout_minutes: 5,
        cleanup_interval_minutes: 5,
        process_startup_timeout_seconds: 30,
        max_restart_attempts: 0,
        http_connection_pool_size: 10,
    };

    SessionMCPManager::new(
        "test-session".to_string(),
        configs,
        config,
        std::env::current_dir().unwrap(),
    )
}

#[test]
fn test_manager_creation() {
    let manager = create_test_manager();
    assert_eq!(manager.session_id, "test-session");
    assert!(manager.has_server("test-server"));
    assert!(!manager.has_server("nonexistent-server"));
}

#[test]
fn test_has_server() {
    let manager = create_test_manager();
    assert!(manager.has_server("test-server"));
    assert!(!manager.has_server("unknown-server"));
}

#[test]
fn test_config_env_vars_are_preserved() {
    let manager = create_test_manager();
    let config = manager.server_configs.get("test-server").unwrap();

    match &config.transport {
        TransportConfig::Stdio { env, .. } => {
            assert_eq!(env.get("TEST_VAR"), Some(&"test_value".to_string()));
        }
        _ => panic!("Expected Stdio transport"),
    }
}

/// Helper to create a test manager that can run real processes
fn create_integration_manager_with_workspace(
    workspace_dir: std::path::PathBuf,
) -> SessionMCPManager {
    let mut configs = HashMap::new();
    let mock_server_path = std::env::current_dir()
        .unwrap()
        .join("tests")
        .join("mock_server.py");
    configs.insert(
        "test-server".to_string(),
        MCPServerConfig {
            name: Some("test-server".to_string()),
            transport: TransportConfig::Stdio {
                command: "python3".to_string(),
                args: vec![mock_server_path.to_string_lossy().to_string()],
                env: HashMap::new(),
            },
            authentication: None,
            metadata: None,
        },
    );

    let config = SessionIsolationConfig {
        idle_timeout_minutes: 5,
        cleanup_interval_minutes: 5,
        process_startup_timeout_seconds: 30,
        max_restart_attempts: 0,
        http_connection_pool_size: 10,
    };

    SessionMCPManager::new("test-session".to_string(), configs, config, workspace_dir)
}

fn create_integration_manager() -> SessionMCPManager {
    create_integration_manager_with_workspace(std::env::current_dir().unwrap())
}

#[tokio::test]
async fn test_lazy_spawn() {
    // Skip if python3 is not available in this environment (CI/CD safety).
    // Using an upfront check avoids silently passing after all assertions are skipped.
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("Skipping integration test: python3 not available");
        return;
    }

    let manager = create_integration_manager();

    // 1. Initial state: No processes
    {
        let processes = manager.active_processes.read().await;
        assert_eq!(
            processes.len(),
            0,
            "Should have practically 0 processes initially"
        );
    }

    // 2. Trigger spawn
    let result = manager.ensure_process_running("test-server").await;
    assert!(
        result.is_ok(),
        "Failed to spawn process: {:?}",
        result.err()
    );

    // 3. Verify process exists
    {
        let processes = manager.active_processes.read().await;
        assert_eq!(processes.len(), 1, "Should have 1 process active");
        assert!(processes.contains_key("test-server"));
    }
}

#[tokio::test]
async fn test_multiple_spawn_attempts_are_serialized() {
    // Skip if python3 is not available. Checking upfront ensures that if the
    // dependency is missing, the test skips cleanly rather than silently passing
    // after all 5 concurrent spawn attempts fail inside the loop.
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("Skipping integration test: python3 not available");
        return;
    }

    let manager = create_integration_manager();

    // Spawn multiple tasks concurrently trying to start the same server
    let mut handles = vec![];
    for _ in 0..5 {
        let m = manager.clone();
        handles.push(tokio::spawn(async move {
            m.ensure_process_running("test-server").await
        }));
    }

    // All tasks must succeed — python3 is confirmed available above
    for h in handles {
        let res = h.await.unwrap();
        assert!(res.is_ok(), "Concurrent spawn failed: {:?}", res.err());
    }

    // Verify serialization: exactly 1 process created despite 5 concurrent attempts
    {
        let processes = manager.active_processes.read().await;
        assert_eq!(
            processes.len(),
            1,
            "Should have exactly 1 process despite concurrent spawn attempts"
        );
    }
}

#[tokio::test]
async fn test_spawn_uses_session_workspace_as_cwd() {
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

    let mut manager = create_integration_manager_with_workspace(workspace_dir.clone());
    if let Some(config) = manager.server_configs.get_mut("test-server") {
        match &mut config.transport {
            TransportConfig::Stdio { env, .. } => {
                env.insert(
                    "MOCK_SERVER_CWD_FILE".to_string(),
                    cwd_capture_file.to_string_lossy().to_string(),
                );
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    let result = manager.ensure_process_running("test-server").await;
    assert!(
        result.is_ok(),
        "Session stdio startup failed: {:?}",
        result.err()
    );

    assert!(
        workspace_dir.exists(),
        "Session workspace should be created before spawning stdio MCP servers"
    );
    assert!(
        cwd_capture_file.exists(),
        "Mock server should capture its working directory"
    );

    let captured_cwd = std::fs::read_to_string(&cwd_capture_file).unwrap();
    assert!(
        std::path::Path::new(captured_cwd.trim()) == workspace_dir.as_path(),
        "Expected stdio MCP server cwd to be '{}', got '{}'",
        workspace_dir.display(),
        captured_cwd.trim()
    );

    {
        let mut processes = manager.active_processes.write().await;
        if let Some(process) = processes.remove("test-server") {
            process.shutdown().await;
        }
    }

    let _ = std::fs::remove_file(cwd_capture_file);
    let _ = std::fs::remove_dir_all(workspace_dir);
}

#[test]
fn test_idle_timeout_configuration() {
    let manager = create_test_manager();
    // Idle timeout should be 5 minutes (300 seconds)
    assert_eq!(manager.idle_timeout, Duration::from_secs(5 * 60));
}

/// Test that environment variables are correctly extracted from config
#[test]
fn test_env_vars_extraction() {
    let mut env_map = HashMap::new();
    env_map.insert("PATH".to_string(), "/custom/path".to_string());
    env_map.insert("CUSTOM_VAR".to_string(), "custom_value".to_string());

    let config = MCPServerConfig {
        name: Some("test".to_string()),
        transport: TransportConfig::Stdio {
            command: "test".to_string(),
            args: vec![],
            env: env_map.clone(),
        },
        authentication: None,
        metadata: None,
    };

    match &config.transport {
        TransportConfig::Stdio { env, .. } => {
            assert_eq!(env.len(), 2);
            assert_eq!(env.get("PATH"), Some(&"/custom/path".to_string()));
            assert_eq!(env.get("CUSTOM_VAR"), Some(&"custom_value".to_string()));
        }
        _ => panic!("Expected Stdio transport"),
    }
}

/// Test that environment isolation is enforced
/// Note: This is a design verification test - we verify that env_clear IS called
#[test]
fn test_env_clear_in_spawn_logic() {
    // This test documents the expected behavior:
    // We MUST call env_clear() to prevent secret leakage from the host process
    // Then we selectively whitelist essential variables like PATH

    let source = include_str!("./lifecycle.rs");

    // Verify that env_clear() IS present in the spawn logic
    assert!(
        source.contains("cmd.env_clear()"),
        "stdio_manager MUST call env_clear() to isolate process environment"
    );

    // Verify that we are whitelisting PATH
    assert!(
        source.contains("\"PATH\""),
        "stdio_manager must whitelist PATH"
    );

    assert!(
        source.contains("cmd.current_dir(&self.workspace_dir)"),
        "stdio_manager must launch session MCP servers from the session workspace"
    );
}

/// Test SessionMCPError variants
#[test]
fn test_error_types() {
    let err1 = SessionMCPError::ServerNotFound("test".to_string());
    assert!(format!("{:?}", err1).contains("ServerNotFound"));

    let err2 = SessionMCPError::SpawnFailed("spawn error".to_string());
    assert!(format!("{:?}", err2).contains("SpawnFailed"));

    let err3 = SessionMCPError::InvalidTransport("wrong type".to_string());
    assert!(format!("{:?}", err3).contains("InvalidTransport"));
}

/// Test that command and args are properly structured
#[test]
fn test_command_args_structure() {
    let mut configs = HashMap::new();
    configs.insert(
        "npx-server".to_string(),
        MCPServerConfig {
            name: Some("npx-server".to_string()),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-example".to_string(),
                ],
                env: HashMap::new(),
            },
            authentication: None,
            metadata: None,
        },
    );

    let config = SessionIsolationConfig {
        idle_timeout_minutes: 5,
        cleanup_interval_minutes: 5,
        process_startup_timeout_seconds: 30,
        max_restart_attempts: 0,
        http_connection_pool_size: 10,
    };

    let manager = SessionMCPManager::new(
        "test".to_string(),
        configs,
        config,
        std::env::current_dir().unwrap(),
    );
    let server_config = manager.server_configs.get("npx-server").unwrap();

    match &server_config.transport {
        TransportConfig::Stdio { command, args, .. } => {
            assert_eq!(command, "npx");
            assert_eq!(args.len(), 2);
            assert_eq!(args[0], "-y");
            assert_eq!(args[1], "@modelcontextprotocol/server-example");
        }
        _ => panic!("Expected Stdio transport"),
    }
}

/// Test session ID tracking
#[test]
fn test_session_id_tracking() {
    let manager = create_test_manager();
    assert_eq!(manager.session_id, "test-session");
}

/// Test that activity tracking structures are initialized
#[tokio::test]
async fn test_activity_tracking_initialization() {
    let manager = create_test_manager();

    let activity = manager.last_activity.read().await;
    assert_eq!(activity.len(), 0, "Activity map should be empty initially");

    let processes = manager.active_processes.read().await;
    assert_eq!(processes.len(), 0, "Process map should be empty initially");
}
