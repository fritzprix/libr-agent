pub mod common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::planning::PlanningServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::schema::JSONSchemaType;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::repositories::SqlitePlanningRepository;
use tauri_mcp_agent_lib::set_planning_repository;

fn extract_text(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn update_todo_schema_leaves_todo_id_unbounded() {
    let update_tool = PlanningServer::tools_static()
        .into_iter()
        .find(|tool| tool.name == "updateTodo")
        .expect("updateTodo tool should exist");

    let properties = match &update_tool.input_schema.schema_type {
        JSONSchemaType::Object {
            properties: Some(properties),
            ..
        } => properties,
        other => panic!("expected object schema, got {other:?}"),
    };

    let todo_id_schema = properties
        .get("todoId")
        .expect("updateTodo should expose todoId");

    match &todo_id_schema.schema_type {
        JSONSchemaType::Integer {
            minimum, maximum, ..
        } => {
            assert_eq!(*minimum, None);
            assert_eq!(*maximum, None);
        }
        other => panic!("expected integer schema, got {other:?}"),
    }
}

#[tokio::test]
async fn planning_context_and_update_todo_use_todo_ids() {
    let db = common::setup_test_db_with_migrations().await;
    set_planning_repository(SqlitePlanningRepository::new(db.clone()));

    let session_id = "planning-todo-id-test";
    let server = PlanningServer::new(session_id.to_string(), Arc::new(db.clone()))
        .await
        .expect("planning server should initialize");

    let add_result = server
        .call_tool(
            "addTodo",
            json!({
                "description": "Write the regression test",
                "priority": "high"
            }),
            None,
        )
        .await
        .expect("addTodo should succeed");

    let structured = add_result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    let todo_id = structured
        .get("todoId")
        .and_then(|value| value.as_i64())
        .expect("todoId should be present");

    let service_context = server.get_service_context(None).await;
    assert!(service_context
        .context_prompt
        .contains(&format!("- #{} [high] Write the regression test", todo_id)));
    assert!(!service_context.context_prompt.contains("| Todo ID |"));
    assert!(!service_context.context_prompt.contains("Use 'todoId'"));

    let update_result = server
        .call_tool(
            "updateTodo",
            json!({
                "todoId": todo_id,
                "action": "done"
            }),
            None,
        )
        .await
        .expect("updateTodo should succeed with todoId");

    let update_text = extract_text(&update_result);
    assert!(update_text.contains(&format!("Todo #{} marked completed", todo_id)));

    let legacy_result = server
        .call_tool(
            "updateTodo",
            json!({
                "index": 0,
                "action": "done"
            }),
            None,
        )
        .await
        .expect("legacy updateTodo call should return a guided error");

    let legacy_text = extract_text(&legacy_result);
    assert!(legacy_text.contains("Missing required parameter: 'todoId'"));
}
