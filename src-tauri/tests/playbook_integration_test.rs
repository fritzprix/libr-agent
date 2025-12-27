use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::playbook::PlaybookServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;

#[tokio::test]
async fn test_playbook_ui_rendering_integration() {
    // Setup in-memory database
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

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

    sqlx::query("INSERT INTO sessions (id, name, status, created_at, updated_at) VALUES ('integration-test', 'Integration Test', 'idle', 0, 0)")
        .execute(&pool)
        .await
        .expect("Failed to insert test session");

    let pool_arc = Arc::new(pool);

    // Create PlaybookServer
    let server = PlaybookServer::new("integration-test".to_string(), pool_arc)
        .await
        .expect("Failed to create PlaybookServer");

    // Save sample playbooks
    server
        .call_tool(
            "savePlaybook",
            json!({
                "id": "workflow-1",
                "title": "Data Processing Workflow",
                "description": "Process and analyze data",
                "template": "1. Load data from {{source}}\n2. Transform with {{method}}\n3. Save to {{destination}}"
            }),
        )
        .await
        .expect("Failed to save playbook 1");

    server
        .call_tool(
            "savePlaybook",
            json!({
                "id": "workflow-2",
                "title": "API Integration Workflow",
                "description": "Connect to external API",
                "template": "1. Authenticate with {{api_key}}\n2. Call endpoint {{endpoint}}\n3. Process response"
            }),
        )
        .await
        .expect("Failed to save playbook 2");

    // Test listPlaybooks with UI rendering
    let list_result = server
        .call_tool("listPlaybooks", json!({}))
        .await
        .expect("Failed to list playbooks");

    assert!(!list_result.is_error.unwrap_or(false));

    // Verify content structure
    let content = list_result.content.unwrap();
    assert_eq!(content.len(), 2, "Expected text and resource content");

    // Verify text content
    if let MCPContent::Text { text } = &content[0] {
        assert!(text.contains("Found 2 playbooks"));
    } else {
        panic!("Expected Text content");
    }

    // Verify UI resource content
    if let MCPContent::Resource { resource } = &content[1] {
        let uri = resource["uri"].as_str().unwrap();
        assert!(uri.contains("ui://playbook/list/integration-test"));

        let mime_type = resource["mimeType"].as_str().unwrap();
        assert_eq!(mime_type, "text/html");

        let html = resource["text"].as_str().unwrap();

        // Verify HTML structure
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html>"));
        assert!(html.contains("📚 Playbooks (2)"));

        // Verify playbook data is rendered
        assert!(html.contains("Data Processing Workflow"));
        assert!(html.contains("Process and analyze data"));
        assert!(html.contains("workflow-1"));

        assert!(html.contains("API Integration Workflow"));
        assert!(html.contains("Connect to external API"));
        assert!(html.contains("workflow-2"));

        // Verify buttons
        assert!(html.contains("btn-select"));
        assert!(html.contains("btn-delete"));

        // Verify JavaScript event handlers
        assert!(html.contains("window.parent.postMessage"));
        assert!(html.contains("builtin_playbook__getPlaybook"));
        assert!(html.contains("builtin_playbook__deletePlaybook"));
    } else {
        panic!("Expected Resource content");
    }

    // Verify structured content
    let structured = list_result.structured_content.unwrap();
    assert_eq!(structured["count"], 2);
    assert_eq!(structured["playbooks"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_playbook_ui_interaction_flow() {
    // Setup
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

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

    sqlx::query("INSERT INTO sessions (id, name, status, created_at, updated_at) VALUES ('flow-test', 'Flow Test', 'idle', 0, 0)")
        .execute(&pool)
        .await
        .expect("Failed to insert test session");

    let server = PlaybookServer::new("flow-test".to_string(), Arc::new(pool))
        .await
        .expect("Failed to create PlaybookServer");

    // Step 1: List empty playbooks (should show empty state)
    let empty_list = server
        .call_tool("listPlaybooks", json!({}))
        .await
        .expect("Failed to list empty playbooks");

    let content = empty_list.content.unwrap();
    if let MCPContent::Resource { resource } = &content[1] {
        let html = resource["text"].as_str().unwrap();
        assert!(html.contains("No playbooks found"));
        assert!(html.contains("Create your first playbook"));
    }

    // Step 2: Save a playbook
    let save_result = server
        .call_tool(
            "savePlaybook",
            json!({
                "id": "test-flow",
                "title": "Test Flow",
                "description": "Test description",
                "template": "Step 1: {{action1}}\nStep 2: {{action2}}"
            }),
        )
        .await
        .expect("Failed to save playbook");

    assert!(!save_result.is_error.unwrap_or(false));

    // Step 3: List again (should show the playbook)
    let list_with_data = server
        .call_tool("listPlaybooks", json!({}))
        .await
        .expect("Failed to list playbooks");

    let content = list_with_data.content.unwrap();
    if let MCPContent::Resource { resource } = &content[1] {
        let html = resource["text"].as_str().unwrap();
        assert!(html.contains("Test Flow"));
        assert!(html.contains("Test description"));
        assert!(html.contains("test-flow"));
    }

    // Step 4: Get specific playbook (simulating Select button click)
    let get_result = server
        .call_tool("getPlaybook", json!({"id": "test-flow"}))
        .await
        .expect("Failed to get playbook");

    assert!(!get_result.is_error.unwrap_or(false));
    let structured = get_result.structured_content.unwrap();
    assert_eq!(structured["title"], "Test Flow");

    // Step 5: Delete playbook (simulating Delete button click)
    let delete_result = server
        .call_tool("deletePlaybook", json!({"id": "test-flow"}))
        .await
        .expect("Failed to delete playbook");

    assert!(!delete_result.is_error.unwrap_or(false));

    // Step 6: List again (should be empty)
    let final_list = server
        .call_tool("listPlaybooks", json!({}))
        .await
        .expect("Failed to list playbooks");

    let structured = final_list.structured_content.unwrap();
    assert_eq!(structured["count"], 0);
}
