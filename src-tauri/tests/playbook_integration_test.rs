use sea_orm::{ConnectionTrait, Database, DatabaseConnection, EntityTrait, Schema, Set};
use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::entity::{playbook, session};
use tauri_mcp_agent_lib::mcp::builtin::playbook::PlaybookServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::repositories::{SqlitePlaybookRepository, SqliteSessionRepository};
use tokio::sync::OnceCell;

// Global test database - initialized once across all tests
static TEST_DB: OnceCell<Arc<DatabaseConnection>> = OnceCell::const_new();

async fn get_or_create_test_db() -> Arc<DatabaseConnection> {
    TEST_DB
        .get_or_init(|| async {
            use sea_orm::ConnectOptions;
            let mut opt =
                ConnectOptions::new("sqlite::file:playbook_tests?mode=memory&cache=shared");
            opt.min_connections(1);
            let db = Database::connect(opt)
                .await
                .expect("Failed to connect to in-memory database");

            let schema = Schema::new(db.get_database_backend());

            // Create sessions table
            let stmt = schema.create_table_from_entity(session::Entity);
            db.execute(db.get_database_backend().build(&stmt))
                .await
                .expect("Failed to create sessions table");

            // Create playbooks table
            let stmt = schema.create_table_from_entity(playbook::Entity);
            db.execute(db.get_database_backend().build(&stmt))
                .await
                .expect("Failed to create playbooks table");

            let db_arc = Arc::new(db);

            // Initialize repositories
            use tauri_mcp_agent_lib::{set_playbook_repository, set_session_repository};
            let session_repo = SqliteSessionRepository::new((*db_arc).clone());
            set_session_repository(session_repo);

            let playbook_repo = SqlitePlaybookRepository::new((*db_arc).clone());
            set_playbook_repository(playbook_repo);

            db_arc
        })
        .await
        .clone()
}

async fn test_playbook_ui_rendering_integration() {
    // Setup shared in-memory database
    let db = get_or_create_test_db().await;

    // Insert test session
    let new_session = session::ActiveModel {
        id: Set("integration-test".to_string()),
        name: Set(Some("Integration Test".to_string())),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        agent_config: Set(Some(
            r#"{"assistant_id":"assistant-rendering-test"}"#.to_string(),
        )),
        status: Set("idle".to_string()),
        parent_session_id: Set(None),
        lineage_id: Set(None),
        depth: Set(None),
        max_depth: Set(None),
        max_fanout: Set(None),
        created_at: Set(0),
        updated_at: Set(0),
        is_bookmarked: Set(false),
        yolo_mode: Set(false),
    };
    session::Entity::insert(new_session)
        .exec(db.as_ref())
        .await
        .expect("Failed to insert test session");

    // Create PlaybookServer
    let server = PlaybookServer::new("integration-test".to_string(), db)
        .await
        .expect("Failed to create PlaybookServer");

    // Save sample playbooks
    server
        .call_tool(
            "createPlaybook",
            json!({
                "goal": "Data Processing Workflow",
                "initialCommand": "process data",
                "workflow": [
                    {
                        "description": "Load data from source",
                        "action": { "toolName": "load_data", "purpose": "Load data" },
                        "outputVariable": "raw_data"
                    },
                    {
                        "description": "Transform data",
                        "action": { "toolName": "transform_data", "purpose": "Transform" },
                        "outputVariable": "processed_data"
                    }
                ],
                "successCriteria": {
                    "description": "Data saved to destination"
                }
            }),
            None,
        )
        .await
        .expect("Failed to save playbook 1");

    server
        .call_tool(
            "createPlaybook",
            json!({
                "goal": "API Integration Workflow",
                "initialCommand": "connect api",
                "workflow": [
                    {
                        "description": "Authenticate",
                        "action": { "toolName": "auth", "purpose": "Login" },
                        "outputVariable": "token"
                    }
                ],
                "successCriteria": {
                    "description": "Connected successfully"
                }
            }),
            None,
        )
        .await
        .expect("Failed to save playbook 2");

    // Test listPlaybooks with UI rendering
    let list_result = server
        .call_tool("showPlaybooks", json!({}), None)
        .await
        .expect("Failed to list playbooks");

    assert!(!list_result.is_error.unwrap_or(false));

    // Verify content structure
    let content = list_result.content.unwrap();
    assert_eq!(content.len(), 2, "Expected text and resource content");

    // Verify text content
    if let MCPContent::Text { text, .. } = &content[0] {
        assert!(text.contains("Data Processing Workflow"));
        assert!(text.contains("API Integration Workflow"));
    } else {
        panic!("Expected Text content");
    }

    // Verify UI resource content
    if let MCPContent::Resource {
        resource,
        service_info: _,
    } = &content[1]
    {
        let uri = resource["uri"].as_str().unwrap();
        // Playbook UI is assistant-scoped; URI should reference the assistant ID
        assert!(uri.contains("ui://playbook/list/assistant-rendering-test"));

        let mime_type = resource["mimeType"].as_str().unwrap();
        assert_eq!(mime_type, "text/html");

        let html = resource["text"].as_str().unwrap();

        // Verify HTML structure
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html>"));
        assert!(html.contains("📚 Playbooks (2)"));

        // Verify playbook data is rendered
        assert!(html.contains("Data Processing Workflow"));
        // assert!(html.contains("Process and analyze data")); // Removed: Not in view model
        // assert!(html.contains("workflow-1")); // Removed: Not in view model

        assert!(html.contains("API Integration Workflow"));
        // assert!(html.contains("Connect to external API")); // Removed: Not in view model
        // assert!(html.contains("workflow-2")); // Removed: Not in view model

        // Verify buttons
        assert!(html.contains("btn-select"));
        assert!(html.contains("btn-delete"));

        // Verify JavaScript event handlers
        assert!(html.contains("window.parent.postMessage"));
        assert!(html.contains("selectPlaybook"));
        assert!(html.contains("deletePlaybook"));
    } else {
        panic!("Expected Resource content");
    }

    // Verify structured content
    let structured = list_result.structured_content.unwrap();
    assert_eq!(structured["page"]["totalItems"], 2);
    assert_eq!(structured["page"]["items"].as_array().unwrap().len(), 2);
}

async fn test_playbook_ui_interaction_flow() {
    // Setup
    let db = get_or_create_test_db().await;

    // Insert test session
    let new_session = session::ActiveModel {
        id: Set("flow-test".to_string()),
        name: Set(Some("Flow Test".to_string())),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        agent_config: Set(Some(
            r#"{"assistant_id":"assistant-flow-test"}"#.to_string(),
        )),
        status: Set("idle".to_string()),
        parent_session_id: Set(None),
        lineage_id: Set(None),
        depth: Set(None),
        max_depth: Set(None),
        max_fanout: Set(None),
        created_at: Set(0),
        updated_at: Set(0),
        is_bookmarked: Set(false),
        yolo_mode: Set(false),
    };
    session::Entity::insert(new_session)
        .exec(db.as_ref())
        .await
        .expect("Failed to insert test session");

    let server = PlaybookServer::new("flow-test".to_string(), db)
        .await
        .expect("Failed to create PlaybookServer");

    // Step 1: List empty playbooks (should show empty state)
    let empty_list = server
        .call_tool("showPlaybooks", json!({}), None)
        .await
        .expect("Failed to list empty playbooks");

    let content = empty_list.content.unwrap();
    if let MCPContent::Resource {
        resource,
        service_info: _,
    } = &content[1]
    {
        let html = resource["text"].as_str().unwrap();
        assert!(html.contains("No playbooks found"));
        assert!(html.contains("Create your first playbook"));
    }

    // Step 2: Save a playbook
    let save_result = server
        .call_tool(
            "createPlaybook",
            json!({
                "goal": "Test Flow",
                "initialCommand": "start flow",
                "workflow": [
                    {
                        "description": "Step 1",
                        "action": { "toolName": "action1", "purpose": "Test" },
                        "outputVariable": "out1"
                    },
                    {
                        "description": "Step 2",
                        "action": { "toolName": "action2", "purpose": "Test" },
                        "outputVariable": "out2"
                    }
                ],
                "successCriteria": {
                    "description": "Flow completed"
                }
            }),
            None,
        )
        .await
        .expect("Failed to save playbook");

    assert!(!save_result.is_error.unwrap_or(false));
    let created_playbook = save_result.structured_content.as_ref().unwrap()["playbook"]
        .as_object()
        .unwrap();
    let playbook_id = created_playbook["id"].as_str().unwrap().to_string();

    // Step 3: List again (should show the playbook)
    let list_with_data = server
        .call_tool("showPlaybooks", json!({}), None)
        .await
        .expect("Failed to list playbooks");

    let content = list_with_data.content.unwrap();
    if let MCPContent::Resource {
        resource,
        service_info: _,
    } = &content[1]
    {
        let html = resource["text"].as_str().unwrap();
        assert!(html.contains("Test Flow"));
        assert!(html.contains(&playbook_id));
    }

    // Step 4: Get specific playbook (simulating Select button click)
    let get_result = server
        .call_tool("getPlaybook", json!({"id": playbook_id}), None)
        .await
        .expect("Failed to get playbook");

    assert!(!get_result.is_error.unwrap_or(false));
    let structured = get_result.structured_content.unwrap();
    assert_eq!(structured["playbook"]["goal"], "Test Flow");

    // Step 5: Delete playbook (simulating Delete button click)
    let delete_result = server
        .call_tool("deletePlaybook", json!({"id": playbook_id}), None)
        .await
        .expect("Failed to delete playbook");

    assert!(!delete_result.is_error.unwrap_or(false));

    // Step 6: List again (should be empty)
    let final_list = server
        .call_tool("listPlaybooks", json!({}), None)
        .await
        .expect("Failed to list playbooks");

    let structured = final_list.structured_content.unwrap();
    assert_eq!(structured["page"]["totalItems"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_playbook_integration_suite() {
    // Run sequentially to avoid race condition on shared memory DB and global repositories
    test_playbook_ui_interaction_flow().await;
    test_playbook_ui_rendering_integration().await;
}
