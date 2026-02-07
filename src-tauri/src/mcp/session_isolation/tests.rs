use super::stdio_manager::SessionMCPManager;
use crate::mcp::session_isolation_config::SessionIsolationConfig;
use crate::mcp::types::{MCPServerConfig, TransportConfig};
use std::collections::HashMap;
use std::time::Duration;

fn create_test_manager() -> SessionMCPManager {
    let mut configs = HashMap::new();
    configs.insert(
        "test-server".to_string(),
        MCPServerConfig {
            name: Some("test-server".to_string()),
            transport: TransportConfig::Stdio {
                command: "python3".to_string(),
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

    SessionMCPManager::new("test-session".to_string(), configs, config)
}

#[tokio::test]
async fn test_lazy_spawn() {
    let manager = create_test_manager();

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
async fn test_concurrent_spawn_race_condition() {
    let manager = create_test_manager();

    // Spawn multiple tasks trying to start the same server
    let mut handles = vec![];
    for _ in 0..5 {
        let m = manager.clone();
        handles.push(tokio::spawn(async move {
            m.ensure_process_running("test-server").await
        }));
    }

    // Wait for all
    for h in handles {
        let res = h.await.unwrap();
        assert!(res.is_ok(), "Concurrent spawn failed: {:?}", res.err());
    }

    // Verify only 1 process created
    {
        let processes = manager.active_processes.read().await;
        assert_eq!(processes.len(), 1, "Should definitely still be 1 process");
    }
}

#[tokio::test]
async fn test_idle_timeout_configuration() {
    let manager = create_test_manager();
    // Idle timeout should be 5 minutes (300 seconds)
    assert_eq!(manager.idle_timeout, Duration::from_secs(5 * 60));
}

#[test]
fn test_has_server() {
    let manager = create_test_manager();
    assert!(manager.has_server("test-server"));
    assert!(!manager.has_server("unknown-server"));
}
