use std::collections::HashMap;
use tauri_mcp_agent_lib::mcp::session_isolation::SessionMCPManager;
use tauri_mcp_agent_lib::mcp::session_isolation_config::SessionIsolationConfig;
use tauri_mcp_agent_lib::mcp::types::{MCPServerConfig, TransportConfig};

fn create_test_manager() -> SessionMCPManager {
    let mut configs = HashMap::new();
    configs.insert(
        "test-server".to_string(),
        MCPServerConfig {
            name: Some("test-server".to_string()),
            transport: TransportConfig::Stdio {
                command: "python3".to_string(),
                // Use absolute path relative to crate root where `cargo test` runs
                args: vec!["tests/mock_server.py".to_string()],
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

    SessionMCPManager::new(
        "test-session".to_string(),
        configs,
        config,
        std::env::current_dir().unwrap(),
    )
}

#[tokio::test]
async fn test_lazy_spawn_integration() {
    let manager = create_test_manager();

    // 1. Initial state: No processes
    assert_eq!(manager.active_process_count().await, 0);

    // 2. Trigger spawn via public API (list_tools)
    let tools = manager.list_tools("test-server").await;
    assert!(tools.is_ok(), "Failed to list tools: {:?}", tools.err());

    // 3. Verify process exists
    assert_eq!(manager.active_process_count().await, 1);
    assert!(manager.is_process_active("test-server").await);
}

#[tokio::test]
async fn test_concurrent_spawn_integration() {
    let manager = create_test_manager();

    // Spawn multiple tasks trying to start the same server via list_tools
    let mut handles = vec![];
    for _ in 0..5 {
        let m = manager.clone();
        handles.push(tokio::spawn(async move {
            m.list_tools("test-server").await
        }));
    }

    // Wait for all
    for h in handles {
        let res = h.await.unwrap();
        assert!(res.is_ok(), "Concurrent list_tools failed: {:?}", res.err());
    }

    // Verify only 1 process created
    assert_eq!(manager.active_process_count().await, 1);
}

#[test]
fn test_has_server_integration() {
    let manager = create_test_manager();
    assert!(manager.has_server("test-server"));
    assert!(!manager.has_server("unknown-server"));
}
