/// Integration tests for MCP Service Proxy Manager and Bootstrap Server
///
/// These tests verify the end-to-end flow of:
/// 1. Creating session-specific proxies
/// 2. Calling builtin tools through proxies
/// 3. Session isolation and cleanup
#[cfg(test)]
mod tests {
    use crate::mcp::server::MCPServerManager;
    use crate::mcp::service_proxy_manager::MCPServiceProxyManager;
    use serde_json::json;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::sync::Arc;

    /// Helper to create a test database pool
    async fn create_test_pool() -> sqlx::SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("Invalid database URL")
            .create_if_missing(true);

        SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .expect("Failed to create test pool")
    }

    #[tokio::test]
    async fn test_proxy_manager_lifecycle() {
        // Create test dependencies
        let pool = create_test_pool().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );
        let mcp_manager = MCPServerManager::new_with_session_manager(session_manager.clone());

        let proxy_manager =
            MCPServiceProxyManager::new(Arc::new(mcp_manager), Arc::new(pool), session_manager);

        // Test 1: Create proxy with bootstrap tool
        let session_id = "test-session-1".to_string();
        let tool_ids = vec!["bootstrap".to_string()];

        let proxy = proxy_manager
            .create_proxy(session_id.clone(), tool_ids, None)
            .await
            .expect("Failed to create proxy");

        assert_eq!(proxy.session_id(), "test-session-1");
        assert_eq!(proxy.builtin_server_count(), 1);
        assert!(proxy.builtin_tool_ids().contains(&"bootstrap".to_string()));

        // Test 2: Get existing proxy
        let retrieved_proxy = proxy_manager
            .get_proxy(&session_id)
            .await
            .expect("Proxy should exist");

        assert_eq!(retrieved_proxy.session_id(), "test-session-1");

        // Test 3: Call detectPlatform tool
        let result = proxy_manager
            .call_tool(
                "test-session-1",
                "builtin_bootstrap__detectPlatform",
                json!({}),
            )
            .await
            .expect("Tool call should succeed");

        assert!(result.result.is_some());
        let mcp_result = result.result.unwrap();
        // Check that content exists in ToolCall result
        match mcp_result {
            crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                assert!(result.content.is_some(), "Content should exist");
                assert!(
                    !result.content.as_ref().unwrap().is_empty(),
                    "Content should not be empty"
                );
            }
            _ => panic!("Expected ToolCall result"),
        }

        // Test 4: Call getBootstrapGuide tool
        let result = proxy_manager
            .call_tool(
                "test-session-1",
                "builtin_bootstrap__getBootstrapGuide",
                json!({
                    "tool": "node",
                    "platform": "auto"
                }),
            )
            .await
            .expect("Tool call should succeed");

        assert!(result.result.is_some());
        let mcp_result = result.result.unwrap();
        // Verify content exists and is_error is false
        match mcp_result {
            crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                assert!(result.content.is_some(), "Content should exist");
                assert!(
                    !result.content.as_ref().unwrap().is_empty(),
                    "Content should not be empty"
                );
                assert!(
                    !result.is_error.unwrap_or(false),
                    "Bootstrap guide should not return error"
                );
            }
            _ => panic!("Expected ToolCall result"),
        }

        // Test 5: Destroy proxy
        proxy_manager.destroy_proxy(&session_id).await;
        assert!(proxy_manager.get_proxy(&session_id).await.is_none());
    }

    #[tokio::test]
    async fn test_session_isolation() {
        // Create test dependencies
        let pool = create_test_pool().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );
        let mcp_manager = MCPServerManager::new_with_session_manager(session_manager.clone());

        let proxy_manager =
            MCPServiceProxyManager::new(Arc::new(mcp_manager), Arc::new(pool), session_manager);

        // Create two sessions
        let session1 = "session-1".to_string();
        let session2 = "session-2".to_string();

        proxy_manager
            .create_proxy(session1.clone(), vec!["bootstrap".to_string()], None)
            .await
            .expect("Failed to create proxy 1");

        proxy_manager
            .create_proxy(session2.clone(), vec!["bootstrap".to_string()], None)
            .await
            .expect("Failed to create proxy 2");

        // Verify both proxies exist independently
        assert_eq!(proxy_manager.proxy_count().await, 2);

        let proxy1 = proxy_manager.get_proxy(&session1).await.unwrap();
        let proxy2 = proxy_manager.get_proxy(&session2).await.unwrap();

        assert_eq!(proxy1.session_id(), "session-1");
        assert_eq!(proxy2.session_id(), "session-2");

        // Both should be able to call tools independently
        let result1 = proxy_manager
            .call_tool("session-1", "builtin_bootstrap__detectPlatform", json!({}))
            .await
            .expect("Session 1 tool call failed");

        let result2 = proxy_manager
            .call_tool("session-2", "builtin_bootstrap__detectPlatform", json!({}))
            .await
            .expect("Session 2 tool call failed");

        assert!(result1.result.is_some());
        assert!(result2.result.is_some());

        // Cleanup
        proxy_manager.destroy_proxy(&session1).await;
        assert_eq!(proxy_manager.proxy_count().await, 1);

        proxy_manager.destroy_proxy(&session2).await;
        assert_eq!(proxy_manager.proxy_count().await, 0);
    }

    #[tokio::test]
    async fn test_concurrent_tool_calls() {
        // Create test dependencies
        let pool = create_test_pool().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );
        let mcp_manager = MCPServerManager::new_with_session_manager(session_manager.clone());

        let proxy_manager = Arc::new(MCPServiceProxyManager::new(
            Arc::new(mcp_manager),
            Arc::new(pool),
            session_manager,
        ));

        // Create session
        proxy_manager
            .create_proxy(
                "concurrent-test".to_string(),
                vec!["bootstrap".to_string()],
                None,
            )
            .await
            .expect("Failed to create proxy");

        // Execute 5 concurrent tool calls
        let mut handles = vec![];

        for i in 0..5 {
            let manager = proxy_manager.clone();
            let tool = if i % 2 == 0 {
                "builtin_bootstrap__detectPlatform"
            } else {
                "builtin_bootstrap__getBootstrapGuide"
            };

            let args = if i % 2 == 0 {
                json!({})
            } else {
                json!({"tool": "node", "platform": "auto"})
            };

            let handle = tokio::spawn(async move {
                manager
                    .call_tool("concurrent-test", tool, args)
                    .await
                    .expect("Tool call failed")
            });

            handles.push(handle);
        }

        // Wait for all to complete
        let mut results = vec![];
        for handle in handles {
            results.push(handle.await);
        }

        // Verify all succeeded
        assert_eq!(results.len(), 5);
        for result in results {
            assert!(result.is_ok());
            let response = result.unwrap();
            assert!(response.result.is_some());
        }

        // Cleanup
        proxy_manager.destroy_proxy("concurrent-test").await;
    }

    #[tokio::test]
    async fn test_error_handling() {
        // Create test dependencies
        let pool = create_test_pool().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );
        let mcp_manager = MCPServerManager::new_with_session_manager(session_manager.clone());

        let proxy_manager =
            MCPServiceProxyManager::new(Arc::new(mcp_manager), Arc::new(pool), session_manager);

        // Test 1: Call tool on non-existent session
        let result = proxy_manager
            .call_tool(
                "nonexistent",
                "builtin_bootstrap__detectPlatform",
                json!({}),
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No proxy found"));

        // Test 2: Create proxy and call unknown tool
        proxy_manager
            .create_proxy(
                "test-errors".to_string(),
                vec!["bootstrap".to_string()],
                None,
            )
            .await
            .expect("Failed to create proxy");

        let result = proxy_manager
            .call_tool("test-errors", "builtin_bootstrap__unknownTool", json!({}))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));

        // Cleanup
        proxy_manager.destroy_proxy("test-errors").await;
    }

    #[tokio::test]
    async fn test_playbook_ui_rendering_integration() {
        // Setup
        let pool = create_test_pool().await;

        // Create sessions table for FK constraint
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT,
                status TEXT DEFAULT 'idle',
                agent_config TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create sessions table");

        sqlx::query("INSERT INTO sessions (id, name, status, created_at, updated_at) VALUES ('playbook-ui-test', 'Test', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert test session");

        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );
        let mcp_manager = MCPServerManager::new_with_session_manager(session_manager.clone());

        let proxy_manager =
            MCPServiceProxyManager::new(Arc::new(mcp_manager), Arc::new(pool), session_manager);

        // Create proxy with playbook tool
        let session_id = "playbook-ui-test".to_string();
        proxy_manager
            .create_proxy(session_id.clone(), vec!["playbook".to_string()], None)
            .await
            .expect("Failed to create proxy");

        // Save sample playbooks
        proxy_manager
            .call_tool(
                &session_id,
                "builtin_playbook__createPlaybook",
                json!({
                    "goal": "Data Processing Workflow",
                    "initialCommand": "process data",
                    "workflow": [
                        {
                            "description": "Load data",
                            "action": { "toolName": "load", "purpose": "load" },
                            "outputVariable": "data"
                        }
                    ],
                    "successCriteria": {
                        "description": "Data processed"
                    }
                }),
            )
            .await
            .expect("Failed to save playbook 1");

        proxy_manager
            .call_tool(
                &session_id,
                "builtin_playbook__createPlaybook",
                json!({
                    "goal": "API Integration",
                    "initialCommand": "connect api",
                    "workflow": [
                        {
                            "description": "Authenticate",
                            "action": { "toolName": "auth", "purpose": "auth" },
                            "outputVariable": "token"
                        }
                    ],
                    "successCriteria": {
                        "description": "Connected"
                    }
                }),
            )
            .await
            .expect("Failed to save playbook 2");

        // Test listPlaybooks with UI rendering
        let response = proxy_manager
            .call_tool(&session_id, "builtin_playbook__showPlaybooks", json!({}))
            .await
            .expect("Failed to list playbooks");

        let result = match response.result.expect("No result in response") {
            crate::mcp::types::MCPResponseResult::ToolCall(result) => result,
            _ => panic!("Expected ToolCall result"),
        };

        assert!(!result.is_error.unwrap_or(false));

        // Verify content structure
        let content = result.content.expect("No content in result");
        assert_eq!(content.len(), 2, "Expected text and resource content");

        // Verify UI resource
        if let crate::mcp::types::MCPContent::Resource { resource, .. } = &content[1] {
            let uri = resource["uri"].as_str().unwrap();
            assert!(uri.contains("ui://playbook/list/"));

            let html = resource["text"].as_str().unwrap();
            assert!(html.contains("<!DOCTYPE html>"));
            assert!(html.contains("📚 Playbooks (2)"));
            assert!(html.contains("Data Processing Workflow"));
            assert!(html.contains("API Integration"));
            assert!(html.contains("btn-select"));
            assert!(html.contains("btn-delete"));
            assert!(html.contains("window.parent.postMessage"));
        } else {
            panic!("Expected Resource content");
        }

        // Verify structured content
        let structured = result.structured_content.expect("No structured content");
        assert_eq!(structured["page"]["totalItems"], 2);

        // Cleanup
        proxy_manager.destroy_proxy(&session_id).await;
    }

    #[tokio::test]
    async fn test_playbook_session_isolation_with_ui() {
        // Setup
        let pool = create_test_pool().await;

        // Create sessions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT,
                status TEXT DEFAULT 'idle',
                agent_config TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create sessions table");

        sqlx::query("INSERT INTO sessions (id, name, status, created_at, updated_at) VALUES ('session-ui-1', 'S1', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert session 1");

        sqlx::query("INSERT INTO sessions (id, name, status, created_at, updated_at) VALUES ('session-ui-2', 'S2', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert session 2");

        let pool_arc = Arc::new(pool);
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );
        let mcp_manager = MCPServerManager::new_with_session_manager(session_manager.clone());

        let proxy_manager =
            MCPServiceProxyManager::new(Arc::new(mcp_manager), pool_arc, session_manager);

        // Create two proxies
        proxy_manager
            .create_proxy(
                "session-ui-1".to_string(),
                vec!["playbook".to_string()],
                None,
            )
            .await
            .expect("Failed to create proxy 1");

        proxy_manager
            .create_proxy(
                "session-ui-2".to_string(),
                vec!["playbook".to_string()],
                None,
            )
            .await
            .expect("Failed to create proxy 2");

        // Save playbook to session 1
        proxy_manager
            .call_tool(
                "session-ui-1",
                "builtin_playbook__createPlaybook",
                json!({
                    "goal": "Session 1 Playbook",
                    "initialCommand": "s1",
                    "workflow": [{
                        "stepId": "step1",
                        "description": "Test step",
                        "action": {
                            "toolName": "testTool",
                            "purpose": "test"
                        },
                        "outputVariable": "result"
                    }],
                    "successCriteria": { "description": "s1" }
                }),
            )
            .await
            .expect("Failed to save to session 1");

        // Save playbook with same ID to session 2
        proxy_manager
            .call_tool(
                "session-ui-2",
                "builtin_playbook__createPlaybook",
                json!({
                    "goal": "Session 2 Playbook",
                    "initialCommand": "s2",
                    "workflow": [{
                        "stepId": "step1",
                        "description": "Test step",
                        "action": {
                            "toolName": "testTool",
                            "purpose": "test"
                        },
                        "outputVariable": "result"
                    }],
                    "successCriteria": { "description": "s2" }
                }),
            )
            .await
            .expect("Failed to save to session 2");

        // List from session 1
        let response1 = proxy_manager
            .call_tool("session-ui-1", "builtin_playbook__showPlaybooks", json!({}))
            .await
            .expect("Failed to list from session 1");

        let result1 = match response1.result.expect("No result") {
            crate::mcp::types::MCPResponseResult::ToolCall(result) => result,
            _ => panic!("Expected ToolCall result"),
        };

        if let crate::mcp::types::MCPContent::Resource { resource, .. } =
            &result1.content.expect("No content")[1]
        {
            let html = resource["text"].as_str().unwrap();
            assert!(html.contains("Session 1 Playbook"));
            assert!(!html.contains("Session 2 Playbook"));
        }

        // List from session 2
        let response2 = proxy_manager
            .call_tool("session-ui-2", "builtin_playbook__showPlaybooks", json!({}))
            .await
            .expect("Failed to list from session 2");

        let result2 = match response2.result.expect("No result") {
            crate::mcp::types::MCPResponseResult::ToolCall(result) => result,
            _ => panic!("Expected ToolCall result"),
        };

        if let crate::mcp::types::MCPContent::Resource { resource, .. } =
            &result2.content.expect("No content")[1]
        {
            let html = resource["text"].as_str().unwrap();
            assert!(html.contains("Session 2 Playbook"));
            assert!(!html.contains("Session 1 Playbook"));
        }

        // Cleanup
        proxy_manager.destroy_proxy("session-ui-1").await;
        proxy_manager.destroy_proxy("session-ui-2").await;
    }
}
