use tauri_mcp_agent_lib::entity::{assistant, mcp_server, playbook, session};
use tauri_mcp_agent_lib::mcp::MCPServiceProxyManager;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Schema};
use serde_json::json;
use std::sync::Arc;

use tauri_mcp_agent_lib::repositories::{
    SqliteAssistantRepository, SqliteContentStoreRepository, SqliteKnowledgeRepository,
    SqliteMCPServerRepository, SqliteMessageRepository, SqlitePlanningRepository,
    SqlitePlaybookRepository, SqliteSessionRepository, SqliteSettingsRepository,
};
use tauri_mcp_agent_lib::{
    set_assistant_repository, set_content_store_repository, set_knowledge_repository,
    set_mcp_server_repository, set_message_repository, set_planning_repository,
    set_playbook_repository, set_session_repository, set_settings_repository,
};
use std::sync::OnceLock;

static TEST_DB: OnceLock<Arc<DatabaseConnection>> = OnceLock::new();

/// Helper to create or get the singleton test database connection
async fn create_test_db() -> Arc<DatabaseConnection> {
    if let Some(db) = TEST_DB.get() {
        return db.clone();
    }

    // Initialize DB with a file to debug persistence issues
    // We use /tmp/libragent_test_proxy.db
    let mut opt =
        sea_orm::ConnectOptions::new("sqlite:///tmp/libragent_test_proxy.db?mode=rwc".to_owned());
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

    let db_clone = db.clone();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        set_mcp_server_repository(SqliteMCPServerRepository::new(db_clone.clone()));
        set_assistant_repository(SqliteAssistantRepository::new(db_clone.clone()));
        set_playbook_repository(SqlitePlaybookRepository::new(db_clone.clone()));
        set_session_repository(SqliteSessionRepository::new(db_clone.clone()));
        set_message_repository(SqliteMessageRepository::new(db_clone.clone()));
        set_content_store_repository(SqliteContentStoreRepository::new(
            db_clone.clone(),
        ));
        set_settings_repository(SqliteSettingsRepository::new(db_clone.clone()));
        set_knowledge_repository(SqliteKnowledgeRepository::new(db_clone.clone()));
        set_planning_repository(SqlitePlanningRepository::new(db_clone.clone()));
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
        tauri_mcp_agent_lib::session::SessionManager::new().expect("Failed to create SessionManager"),
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
        tauri_mcp_agent_lib::mcp::types::MCPResponseResult::ToolCall(ref result) => {
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
        tauri_mcp_agent_lib::mcp::types::MCPResponseResult::ToolCall(ref result) => {
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
        tauri_mcp_agent_lib::session::SessionManager::new().expect("Failed to create SessionManager"),
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
    let db = create_test_db().await;
    let session_manager = Arc::new(
        tauri_mcp_agent_lib::session::SessionManager::new().expect("Failed to create SessionManager"),
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
    let db = create_test_db().await;
    let session_manager = Arc::new(
        tauri_mcp_agent_lib::session::SessionManager::new().expect("Failed to create SessionManager"),
    );

    let proxy_manager = MCPServiceProxyManager::new(db, session_manager);

    // Test 1: Call tool on non-existent session
    let result = proxy_manager
        .call_tool(
            "nonexistent",
            "builtin_bootstrap__detectPlatform",
            json!({}),
        )
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
        .call_tool("test-errors", "builtin_bootstrap__unknownTool", json!({}))
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    // "Unknown tool" or "Tool not found" depending on normalization
    assert!(err.contains("Unknown tool") || err.contains("Invalid tool"));

    // Cleanup
    proxy_manager.destroy_proxy("test-errors").await;
}

// I will skip playbook tests for now as they require more complex DB setup (active model usage).
// I focus on routing tests.
