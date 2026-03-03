/// Integration tests for MCP Service Proxy Manager and Bootstrap Server
///
/// These tests verify the end-to-end flow of:
/// 1. Creating session-specific proxies
/// 2. Calling builtin tools through proxies
/// 3. Session isolation and cleanup
#[cfg(test)]
mod tests {
    use crate::entity::{assistant, mcp_server, playbook, session};
    use crate::mcp::service_proxy_manager::MCPServiceProxyManager;
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, EntityTrait, Schema, Set};
    use serde_json::json;
    use std::sync::Arc;

    use crate::repositories::{
        SqliteAssistantRepository, SqliteContentStoreRepository, SqliteKnowledgeRepository,
        SqliteMCPServerRepository, SqliteMessageRepository, SqlitePlanningRepository,
        SqlitePlaybookRepository, SqliteSessionRepository, SqliteSettingsRepository,
    };
    use crate::state;
    use std::sync::OnceLock;

    static TEST_DB: OnceLock<Arc<DatabaseConnection>> = OnceLock::new();

    /// Helper to create or get the singleton test database connection
    async fn create_test_db() -> Arc<DatabaseConnection> {
        if let Some(db) = TEST_DB.get() {
            return db.clone();
        }

        // Initialize DB with a file to debug persistence issues
        // We use /tmp/libragent_test.db
        let mut opt =
            sea_orm::ConnectOptions::new("sqlite:///tmp/libragent_test.db?mode=rwc".to_owned());
        opt.max_connections(1);
        let db = Database::connect(opt)
            .await
            .expect("Failed to connect to file database");

        let schema = Schema::new(db.get_database_backend());

        // Create sessions table
        let stmt = schema.create_table_from_entity(session::Entity);
        if let Err(e) = db.execute(db.get_database_backend().build(&stmt)).await {
            // Ignore error if table exists (race condition)
            if !e.to_string().contains("already exists") {
                panic!("Failed to create sessions table: {}", e);
            }
        }

        // Create playbooks table
        let stmt = schema.create_table_from_entity(playbook::Entity);
        if let Err(e) = db.execute(db.get_database_backend().build(&stmt)).await {
            if !e.to_string().contains("already exists") {
                panic!("Failed to create playbooks table: {}", e);
            }
        }

        // Create assistants table
        let stmt = schema.create_table_from_entity(assistant::Entity);
        if let Err(e) = db.execute(db.get_database_backend().build(&stmt)).await {
            if !e.to_string().contains("already exists") {
                panic!("Failed to create assistants table: {}", e);
            }
        }

        // Create mcp_servers table
        let stmt = schema.create_table_from_entity(mcp_server::Entity);
        if let Err(e) = db.execute(db.get_database_backend().build(&stmt)).await {
            if !e.to_string().contains("already exists") {
                panic!("Failed to create mcp_servers table: {}", e);
            }
        }

        // Initialize global repositories
        // We capture panics because they panic if already set (race condition handled via OnceLock check usually,
        // but parallel tests might race between the check and the set in state.rs?
        // No, state.rs uses OnceLock too. If we are the first to run create_test_db, we win.
        // But create_test_db can be called concurrently.
        // OnceLock::get_or_init is what we want, but it's blocking and we have async code.
        // We will do a best effort initialization.

        // We can't use OnceLock::get_or_init with async.
        // We will just initialize and try to set.
        // If state is already set, it's fine.

        let db_clone = db.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            // We ignore failures here, assuming they failed because they are already set.
            // Using a separate thread to catch panic might be safer if panic=abort is not set.
            // But sea_orm::DatabaseConnection is Send+Sync.

            // However, state.rs setters PANIC explicitely.
            // We should use a helper that checks if set?
            // state.rs only exposes set_... and get_...  (get panics if not set).
            // It doesn't expose "is_set".
            // We will trust the catch_unwind.
            state::set_mcp_server_repository(SqliteMCPServerRepository::new(db_clone.clone()));
            state::set_assistant_repository(SqliteAssistantRepository::new(db_clone.clone()));
            state::set_playbook_repository(SqlitePlaybookRepository::new(db_clone.clone()));
            state::set_session_repository(SqliteSessionRepository::new(db_clone.clone()));
            state::set_message_repository(SqliteMessageRepository::new(db_clone.clone()));
            state::set_content_store_repository(SqliteContentStoreRepository::new(
                db_clone.clone(),
            ));
            state::set_settings_repository(SqliteSettingsRepository::new(db_clone.clone()));
            state::set_knowledge_repository(SqliteKnowledgeRepository::new(db_clone.clone()));
            state::set_planning_repository(SqlitePlanningRepository::new(db_clone.clone()));
        }));

        // Store in local static (best effort race)
        let _ = TEST_DB.set(Arc::new(db));

        TEST_DB.get().expect("DB should be set").clone()
    }

    #[tokio::test]
    async fn test_proxy_manager_lifecycle() {
        // Create test dependencies
        let db = create_test_db().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );

        let proxy_manager = MCPServiceProxyManager::new(db, session_manager);

        // Test 1: Create proxy with bootstrap tool
        let session_id = "test-session-1".to_string();
        let tool_ids = vec!["bootstrap".to_string()];

        let proxy = proxy_manager
            .create_proxy(session_id.clone(), tool_ids, vec![], None)
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
            .call_tool("test-session-1", "bootstrap__detectPlatform", json!({}))
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
                "bootstrap__getBootstrapGuide",
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
        let db = create_test_db().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );

        let proxy_manager = MCPServiceProxyManager::new(db, session_manager);

        // Create two sessions
        let session1 = "session-1".to_string();
        let session2 = "session-2".to_string();

        proxy_manager
            .create_proxy(
                session1.clone(),
                vec!["bootstrap".to_string()],
                vec![],
                None,
            )
            .await
            .expect("Failed to create proxy 1");

        proxy_manager
            .create_proxy(
                session2.clone(),
                vec!["bootstrap".to_string()],
                vec![],
                None,
            )
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
            .call_tool("session-1", "bootstrap__detectPlatform", json!({}))
            .await
            .expect("Session 1 tool call failed");

        let result2 = proxy_manager
            .call_tool("session-2", "bootstrap__detectPlatform", json!({}))
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
        let db = create_test_db().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );

        let proxy_manager = Arc::new(MCPServiceProxyManager::new(db, session_manager));

        // Create session
        proxy_manager
            .create_proxy(
                "concurrent-test".to_string(),
                vec!["bootstrap".to_string()],
                vec![],
                None,
            )
            .await
            .expect("Failed to create proxy");

        // Execute 5 concurrent tool calls
        let mut handles = vec![];

        for i in 0..5 {
            let manager = proxy_manager.clone();
            let tool = if i % 2 == 0 {
                "bootstrap__detectPlatform"
            } else {
                "bootstrap__getBootstrapGuide"
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
        let db = create_test_db().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );

        let proxy_manager = MCPServiceProxyManager::new(db, session_manager);

        // Test 1: Call tool on non-existent session
        let result = proxy_manager
            .call_tool("nonexistent", "bootstrap__detectPlatform", json!({}))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Session context not found"));

        // Test 2: Create proxy and call unknown tool
        proxy_manager
            .create_proxy(
                "test-errors".to_string(),
                vec!["bootstrap".to_string()],
                vec![],
                None,
            )
            .await
            .expect("Failed to create proxy");

        let result = proxy_manager
            .call_tool("test-errors", "bootstrap__unknownTool", json!({}))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));

        // Cleanup
        proxy_manager.destroy_proxy("test-errors").await;
    }

    #[tokio::test]
    async fn test_playbook_ui_rendering_integration() {
        // Setup
        let db = create_test_db().await;

        // Insert test session
        let new_session = session::ActiveModel {
            id: Set("playbook-ui-test".to_string()),
            name: Set(Some("Test".to_string())),
            status: Set("idle".to_string()),
            agent_config: Set(Some(json!({ "assistantId": "asst-s1" }).to_string())),
            created_at: Set(0),
            updated_at: Set(0),
            ..Default::default()
        };
        // Defensive cleanup
        let _ = session::Entity::delete_by_id("playbook-ui-test".to_string())
            .exec(db.as_ref())
            .await;
        session::Entity::insert(new_session)
            .exec(db.as_ref())
            .await
            .expect("Failed to insert test session");

        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );

        let proxy_manager = MCPServiceProxyManager::new(db, session_manager);

        // Create proxy with playbook tool
        let session_id = "playbook-ui-test".to_string();
        proxy_manager
            .create_proxy(
                session_id.clone(),
                vec!["playbook".to_string()],
                vec![],
                None,
            )
            .await
            .expect("Failed to create proxy");

        // Save sample playbooks
        proxy_manager
            .call_tool(
                &session_id,
                "playbook__createPlaybook",
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
                "playbook__createPlaybook",
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
            .call_tool(&session_id, "playbook__showPlaybooks", json!({}))
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
        let db = create_test_db().await;

        // Insert test sessions
        let s1 = session::ActiveModel {
            id: Set("session-ui-1".to_string()),
            name: Set(Some("S1".to_string())),
            status: Set("idle".to_string()),
            agent_config: Set(Some(json!({ "assistantId": "asst-ui-test" }).to_string())),
            created_at: Set(0),
            updated_at: Set(0),
            ..Default::default()
        };
        // Defensive cleanup
        let _ = session::Entity::delete_by_id("session-ui-1".to_string())
            .exec(db.as_ref())
            .await;
        session::Entity::insert(s1)
            .exec(db.as_ref())
            .await
            .expect("Failed to insert session 1");

        let s2 = session::ActiveModel {
            id: Set("session-ui-2".to_string()),
            name: Set(Some("S2".to_string())),
            status: Set("idle".to_string()),
            agent_config: Set(Some(json!({ "assistantId": "asst-s1" }).to_string())),
            created_at: Set(0),
            updated_at: Set(0),
            ..Default::default()
        };
        // Defensive cleanup
        let _ = session::Entity::delete_by_id("session-ui-2".to_string())
            .exec(db.as_ref())
            .await;
        session::Entity::insert(s2)
            .exec(db.as_ref())
            .await
            .expect("Failed to insert session 2");

        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );

        let proxy_manager = MCPServiceProxyManager::new(db, session_manager);

        // Create two proxies
        proxy_manager
            .create_proxy(
                "session-ui-1".to_string(),
                vec!["playbook".to_string()],
                vec![],
                None,
            )
            .await
            .expect("Failed to create proxy 1");

        proxy_manager
            .create_proxy(
                "session-ui-2".to_string(),
                vec!["playbook".to_string()],
                vec![],
                None,
            )
            .await
            .expect("Failed to create proxy 2");

        // Save playbook to session 1
        proxy_manager
            .call_tool(
                "session-ui-1",
                "playbook__createPlaybook",
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
                "playbook__createPlaybook",
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
            .call_tool("session-ui-1", "playbook__showPlaybooks", json!({}))
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
            .call_tool("session-ui-2", "playbook__showPlaybooks", json!({}))
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

    #[tokio::test]
    async fn test_shell_workspace_path_resolution() {
        // Create test dependencies
        let db = create_test_db().await;
        let session_manager = Arc::new(
            crate::session::SessionManager::new().expect("Failed to create SessionManager"),
        );

        let proxy_manager = MCPServiceProxyManager::new(db, session_manager);

        let session_id = "path_resolution_test_session".to_string();

        // Create proxy with workspace tools
        proxy_manager
            .create_proxy(
                session_id.clone(),
                vec!["workspace".to_string()],
                vec![],
                None,
            )
            .await
            .expect("Failed to create proxy");

        // Execute 'pwd' command via runInPersistentShell
        let response = proxy_manager
            .call_tool(
                &session_id,
                "workspace__runInPersistentShell",
                json!({
                    "command": "pwd",
                    "runMode": "sync"
                }),
            )
            .await
            .expect("Tool call failed");

        let result = match response.result.expect("No result") {
            crate::mcp::types::MCPResponseResult::ToolCall(result) => result,
            _ => panic!("Expected ToolCall result"),
        };

        // Parse persistent shell output (JSON structured data)
        let structured_content = result.structured_content.expect("No structured content");
        let stdout = structured_content["stdout"].as_str().expect("No stdout");

        // Verify the path contains the session ID, confirming it's in the correct isolated workspace
        assert!(
            stdout.contains(&session_id),
            "Shell CWD should contain session ID '{}', got: '{}'",
            session_id,
            stdout
        );

        // Verify it does NOT contain 'default' (unless the session ID is default, which it isn't)
        assert!(
            !stdout.contains("default") || stdout.contains("workspaces/default"), // default could be in base path but not as session dir if we used a specific one
            "Shell CWD should NOT be the default workspace, got: '{}'",
            stdout
        );

        // Cleanup
        proxy_manager.destroy_proxy(&session_id).await;
    }
}
