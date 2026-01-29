use super::*;
use crate::entity::{
    assistant, knowledge, planning_goal, planning_scratchpad, planning_todo, playbook, session,
};
use sea_orm::{ConnectionTrait, Database, EntityTrait, Schema, Set};
use serde_json::json;

async fn create_test_manager() -> Arc<MCPServiceProxyManager> {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory database");

    let schema = Schema::new(db.get_database_backend());

    // Create tables
    let stmt = schema.create_table_from_entity(session::Entity);
    db.execute(db.get_database_backend().build(&stmt))
        .await
        .expect("Failed to create session table");

    let stmt = schema.create_table_from_entity(playbook::Entity);
    db.execute(db.get_database_backend().build(&stmt))
        .await
        .expect("Failed to create playbook table");

    let stmt = schema.create_table_from_entity(assistant::Entity);
    db.execute(db.get_database_backend().build(&stmt))
        .await
        .expect("Failed to create assistant table");

    let stmt = schema.create_table_from_entity(knowledge::Entity);
    db.execute(db.get_database_backend().build(&stmt))
        .await
        .expect("Failed to create knowledge table");

    let stmt = schema.create_table_from_entity(planning_goal::Entity);
    db.execute(db.get_database_backend().build(&stmt))
        .await
        .expect("Failed to create planning_goal table");

    let stmt = schema.create_table_from_entity(planning_todo::Entity);
    db.execute(db.get_database_backend().build(&stmt))
        .await
        .expect("Failed to create planning_todo table");

    let stmt = schema.create_table_from_entity(planning_scratchpad::Entity);
    db.execute(db.get_database_backend().build(&stmt))
        .await
        .expect("Failed to create planning_scratchpad table");

    // Create a minimal SessionManager
    let session_manager = Arc::new(crate::session::SessionManager::new().unwrap());

    Arc::new(MCPServiceProxyManager::new(Arc::new(db), session_manager))
}

#[tokio::test]
async fn test_phase3_playbook_and_assistant_integration() {
    let manager = create_test_manager().await;

    // Create session 1 with all Phase 3 tools
    let session1 = "test-session-1".to_string();
    let tool_ids1 = vec!["playbook".to_string(), "assistant".to_string()];

    // Insert session 1 into sessions table
    let new_session = session::ActiveModel {
        id: Set(session1.clone()),
        created_at: Set(chrono::Utc::now().timestamp()),
        updated_at: Set(0),
        status: Set("idle".to_string()),
        ..Default::default()
    };
    session::Entity::insert(new_session)
        .exec(&*manager.db)
        .await
        .unwrap();

    manager
        .create_proxy(session1.clone(), tool_ids1, vec![], None)
        .await
        .unwrap();

    // Test 1: Save a playbook in session 1
    let playbook_result = manager
        .call_tool(
            &session1,
            "builtin_playbook__createPlaybook",
            json!({
                "goal": "Test Workflow",
                "initialCommand": "test",
                "workflow": [
                    {
                        "description": "Step 1",
                        "action": { "toolName": "test", "purpose": "test" },
                        "outputVariable": "out"
                    }
                ],
                "successCriteria": {
                    "description": "Success"
                }
            }),
        )
        .await
        .unwrap();

    assert!(
        playbook_result.error.is_none(),
        "Playbook save should succeed"
    );

    // Test 2: Create an assistant (global scope)
    let assistant_result = manager
        .call_tool(
            &session1,
            "builtin_assistant__createAssistant",
            json!({
                "id": "assistant1",
                "name": "Test Assistant",
                "config": json!({
                    "model": "gpt-4",
                    "temperature": 0.7
                })
            }),
        )
        .await
        .unwrap();

    assert!(
        assistant_result.error.is_none(),
        "Assistant create should succeed"
    );

    // Create session 2 with same tools
    let session2 = "test-session-2".to_string();
    let tool_ids2 = vec!["playbook".to_string(), "assistant".to_string()];

    // Insert session 2 into sessions table
    let new_session = session::ActiveModel {
        id: Set(session2.clone()),
        created_at: Set(chrono::Utc::now().timestamp()),
        updated_at: Set(0),
        status: Set("idle".to_string()),
        ..Default::default()
    };
    session::Entity::insert(new_session)
        .exec(&*manager.db)
        .await
        .unwrap();

    manager
        .create_proxy(session2.clone(), tool_ids2, vec![], None)
        .await
        .unwrap();

    // Test 3: Verify playbook isolation (session 2 can't see session 1's playbook)
    let list_result = manager
        .call_tool(&session2, "builtin_playbook__listPlaybooks", json!({}))
        .await
        .unwrap();

    assert!(list_result.error.is_none());
    let result_data = list_result.result.unwrap();

    // Extract text content from ToolCall result
    let text_content = match result_data {
        crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
            if let Some(content) = &result.content {
                if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                    text
                } else {
                    panic!("Expected Text content")
                }
            } else {
                panic!("Expected content")
            }
        }
        _ => panic!("Expected ToolCall result"),
    };
    assert!(
        text_content.contains("No playbooks found"),
        "Session 2 should have 0 playbooks, got: {}",
        text_content
    );

    // Test 4: Verify assistant is global (session 2 can see the assistant)
    let get_assistant_result = manager
        .call_tool(
            &session2,
            "builtin_assistant__getAssistant",
            json!({
                "id": "assistant1"
            }),
        )
        .await
        .unwrap();

    assert!(get_assistant_result.error.is_none());
    let assistant = get_assistant_result.result.unwrap();
    let assistant_text = match assistant {
        crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
            if let Some(content) = &result.content {
                if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                    text
                } else {
                    panic!("Expected Text content")
                }
            } else {
                panic!("Expected content")
            }
        }
        _ => panic!("Expected ToolCall result"),
    };
    assert!(
        assistant_text.contains("Test Assistant"),
        "Session 2 should see the global assistant"
    );

    // Test 5: Save same playbook ID in session 2 (allowed due to composite PK)
    let playbook2_result = manager
        .call_tool(
            &session2,
            "builtin_playbook__createPlaybook",
            json!({
                "goal": "Session 2 Workflow",
                "initialCommand": "test2",
                "workflow": [
                    {
                        "description": "Step 1",
                        "action": { "toolName": "test", "purpose": "test" },
                        "outputVariable": "out"
                    }
                ],
                "successCriteria": {
                    "description": "Success"
                }
            }),
        )
        .await
        .unwrap();

    assert!(
        playbook2_result.error.is_none(),
        "Session 2 should save playbook with same ID"
    );

    // Test 6: Verify each session sees its own playbook
    let list_playbook1 = manager
        .call_tool(&session1, "builtin_playbook__listPlaybooks", json!({}))
        .await
        .unwrap();

    let playbook1_result = list_playbook1.result.unwrap();
    let playbook1_text = match playbook1_result {
        crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
            if let Some(content) = &result.content {
                if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                    text
                } else {
                    panic!("Expected Text content")
                }
            } else {
                panic!("Expected content")
            }
        }
        _ => panic!("Expected ToolCall result"),
    };
    assert!(
        playbook1_text.contains("Test Workflow"),
        "Session 1 should see its own playbook"
    );

    let list_playbook2 = manager
        .call_tool(&session2, "builtin_playbook__listPlaybooks", json!({}))
        .await
        .unwrap();

    let playbook2_result = list_playbook2.result.unwrap();
    let playbook2_text = match playbook2_result {
        crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
            if let Some(content) = &result.content {
                if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                    text
                } else {
                    panic!("Expected Text content")
                }
            } else {
                panic!("Expected content")
            }
        }
        _ => panic!("Expected ToolCall result"),
    };
    assert!(
        playbook2_text.contains("Session 2 Workflow"),
        "Session 2 should see its own playbook"
    );

    // Cleanup
    manager.destroy_proxy(&session1).await;
    manager.destroy_proxy(&session2).await;
}

#[tokio::test]
async fn test_phase3_concurrent_operations() {
    let manager = create_test_manager().await;

    // Create 3 concurrent sessions
    let sessions = vec![
        "concurrent-1".to_string(),
        "concurrent-2".to_string(),
        "concurrent-3".to_string(),
    ];

    // Insert sessions into database and create proxies
    for session_id in &sessions {
        let new_session = session::ActiveModel {
            id: Set(session_id.clone()),
            created_at: Set(chrono::Utc::now().timestamp()),
            updated_at: Set(0),
            status: Set("idle".to_string()),
            ..Default::default()
        };
        session::Entity::insert(new_session)
            .exec(&*manager.db)
            .await
            .unwrap();

        let tool_ids = vec!["playbook".to_string(), "assistant".to_string()];
        manager
            .create_proxy(session_id.clone(), tool_ids, vec![], None)
            .await
            .unwrap();
    }

    // Execute concurrent playbook saves
    let mut handles = vec![];
    for (idx, session_id) in sessions.iter().enumerate() {
        let mgr = manager.clone();
        let sid = session_id.clone();

        let handle = tokio::spawn(async move {
            mgr.call_tool(
                &sid,
                "builtin_playbook__createPlaybook",
                json!({
                    "goal": format!("Playbook {}", idx),
                    "initialCommand": format!("test {}", idx),
                    "workflow": [
                        {
                            "description": format!("Step {}", idx),
                            "action": { "toolName": "test", "purpose": "test" },
                            "outputVariable": "out"
                        }
                    ],
                    "successCriteria": {
                        "description": "Success"
                    }
                }),
            )
            .await
        });

        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        let result = handle.await.unwrap().unwrap();
        assert!(
            result.error.is_none(),
            "Concurrent playbook save should succeed"
        );
    }

    // Verify each session has its own playbooks
    for (idx, session_id) in sessions.iter().enumerate() {
        let list_result = manager
            .call_tool(session_id, "builtin_playbook__listPlaybooks", json!({}))
            .await
            .unwrap();

        let result_data = list_result.result.unwrap();
        let text_content = match result_data {
            crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                if let Some(content) = &result.content {
                    if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                        text
                    } else {
                        panic!("Expected Text content")
                    }
                } else {
                    panic!("Expected content")
                }
            }
            _ => panic!("Expected ToolCall result"),
        };

        // Each session should have exactly 1 playbook
        assert!(
            text_content.contains("Found 1 playbook"),
            "Session {} should have exactly 1 playbook, got: {}",
            idx,
            text_content
        );
    }

    // Cleanup
    for session_id in &sessions {
        manager.destroy_proxy(session_id).await;
    }
}

#[tokio::test]
async fn test_phase3_all_servers_integration() {
    let manager = create_test_manager().await;

    let session_id = "integration-test".to_string();

    // Insert session into database
    use crate::entity::session;
    use sea_orm::Set;

    let new_session = session::ActiveModel {
        id: Set(session_id.clone()),
        created_at: Set(chrono::Utc::now().timestamp()),
        updated_at: Set(0),
        status: Set("idle".to_string()),
        ..Default::default()
    };
    session::Entity::insert(new_session)
        .exec(&*manager.db)
        .await
        .unwrap();

    // Create proxy with ALL builtin servers
    let all_tools = vec![
        "bootstrap".to_string(),
        "knowledge".to_string(),
        "planning".to_string(),
        "playbook".to_string(),
        "assistant".to_string(),
    ];

    manager
        .create_proxy(session_id.clone(), all_tools, vec![], None)
        .await
        .unwrap();

    // Test Bootstrap (stateless)
    let bootstrap_result = manager
        .call_tool(&session_id, "builtin_bootstrap__detectPlatform", json!({}))
        .await
        .unwrap();
    assert!(bootstrap_result.error.is_none(), "Bootstrap should work");

    // Test Knowledge (session-scoped)
    let knowledge_result = manager
        .call_tool(
            &session_id,
            "builtin_content_store__addContent",
            json!({
                "title": "Test Knowledge",
                "content": "Integration test content",
                "tags": ["test", "integration"]
            }),
        )
        .await
        .unwrap();
    assert!(
        knowledge_result.error.is_none(),
        "Knowledge save should work"
    );

    // Test Planning (session-scoped)
    let planning_result = manager
        .call_tool(
            &session_id,
            "builtin_planning__createGoal",
            json!({
                "goal": "Complete Phase 3 integration"
            }),
        )
        .await
        .unwrap();
    assert!(planning_result.error.is_none(), "Planning should work");

    // Test Playbook (session-scoped)
    let playbook_result = manager
        .call_tool(
            &session_id,
            "builtin_playbook__createPlaybook",
            json!({
                "goal": "Integration Playbook",
                "initialCommand": "test",
                "workflow": [
                    {
                        "description": "Step 1",
                        "action": { "toolName": "test", "purpose": "test" },
                        "outputVariable": "out"
                    }
                ],
                "successCriteria": {
                    "description": "Success"
                }
            }),
        )
        .await
        .unwrap();
    assert!(playbook_result.error.is_none(), "Playbook should work");

    // Test Assistant (global-scoped)
    let assistant_result = manager
        .call_tool(
            &session_id,
            "builtin_assistant__createAssistant",
            json!({
                "id": "integration-assistant",
                "name": "Integration Test Assistant",
                "config": json!({ "model": "test" })
            }),
        )
        .await
        .unwrap();
    assert!(assistant_result.error.is_none(), "Assistant should work");

    // Verify proxy has all servers
    let proxy = manager.get_proxy(&session_id).await.unwrap();
    assert_eq!(
        proxy.builtin_server_count(),
        5,
        "Should have all 5 builtin servers"
    );

    // Cleanup
    manager.destroy_proxy(&session_id).await;
    assert_eq!(
        manager.proxy_count().await,
        0,
        "All proxies should be destroyed"
    );
}

#[tokio::test]
async fn test_empty_mcp_server_ids_means_no_external_servers() {
    let manager = create_test_manager().await;

    let session_id = "no-external-test".to_string();

    // Insert session into database
    use crate::entity::session;
    use sea_orm::Set;

    let new_session = session::ActiveModel {
        id: Set(session_id.clone()),
        created_at: Set(chrono::Utc::now().timestamp()),
        updated_at: Set(0),
        status: Set("idle".to_string()),
        ..Default::default()
    };
    session::Entity::insert(new_session)
        .exec(&*manager.db)
        .await
        .unwrap();

    // Create proxy with builtin tools but EMPTY mcp_server_ids
    // This should result in NO external MCP servers being loaded
    let tool_ids = vec!["bootstrap".to_string()];
    let mcp_server_ids = vec![]; // Empty = no external servers

    let proxy = manager
        .create_proxy(session_id.clone(), tool_ids, mcp_server_ids, None)
        .await
        .unwrap();

    // Verify: Only builtin tools, no external tools
    assert_eq!(
        proxy.builtin_server_count(),
        1,
        "Should have 1 builtin server"
    );

    // Verify: No external stdio tools
    let stdio_tools = proxy.get_session_stdio_tools().await;
    assert_eq!(
        stdio_tools.len(),
        0,
        "Should have 0 external stdio tools (mcp_server_ids is empty)"
    );

    // Verify: No external HTTP tools
    let http_tools = proxy.get_session_http_tools().await;
    assert_eq!(
        http_tools.len(),
        0,
        "Should have 0 external HTTP tools (mcp_server_ids is empty)"
    );

    log::info!("✅ Verified: Empty mcp_server_ids = no external servers loaded");

    // Cleanup
    manager.destroy_proxy(&session_id).await;
}
