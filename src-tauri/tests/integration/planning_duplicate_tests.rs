use crate::common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::planning::PlanningServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::repositories::{PlanningRepository, SqlitePlanningRepository};
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

#[tokio::test]
async fn add_todo_rejects_duplicate_title_with_error_semantics() {
    let db = common::setup_test_db_with_migrations().await;
    set_planning_repository(SqlitePlanningRepository::new(db.clone()));

    let session_id = "planning-duplicate-todo";
    let server = PlanningServer::new(session_id.to_string(), Arc::new(db.clone()))
        .await
        .expect("planning server should initialize");

    server
        .call_tool(
            "addTodo",
            json!({
                "description": "Write the regression test",
                "priority": "high"
            }),
            None,
        )
        .await
        .expect("first addTodo should succeed");

    let duplicate = server
        .call_tool(
            "addTodo",
            json!({
                "description": "write the regression test",
                "priority": "medium"
            }),
            None,
        )
        .await
        .expect("duplicate addTodo should return an MCP result");

    let text = extract_text(&duplicate);
    assert_eq!(duplicate.is_error, Some(true));
    assert!(
        text.contains("Todo 'write the regression test' already exists"),
        "expected duplicate warning in text content, got: {text}"
    );
}

#[tokio::test]
async fn create_goal_rejects_identical_active_goal_with_error_semantics() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqlitePlanningRepository::new(db.clone());
    set_planning_repository(repo);

    let session_id = "planning-duplicate-goal";
    let server = PlanningServer::new(session_id.to_string(), Arc::new(db.clone()))
        .await
        .expect("planning server should initialize");

    server
        .call_tool(
            "createGoal",
            json!({ "goal": "Ship duplicate prevention" }),
            None,
        )
        .await
        .expect("first createGoal should succeed");

    let duplicate = server
        .call_tool(
            "createGoal",
            json!({ "goal": "ship duplicate prevention" }),
            None,
        )
        .await
        .expect("duplicate createGoal should return an MCP result");

    let text = extract_text(&duplicate);
    assert_eq!(duplicate.is_error, Some(true));
    assert!(
        text.contains("The active goal is already 'Ship duplicate prevention'"),
        "expected duplicate goal warning in text content, got: {text}"
    );
    assert!(
        text.contains("No new goal was created"),
        "expected explicit no-op warning, got: {text}"
    );

    let planning_repo = SqlitePlanningRepository::new(db);
    let active = planning_repo
        .get_active_goal(session_id)
        .await
        .expect("active goal lookup should succeed")
        .expect("one active goal should remain");
    assert_eq!(active.goal_text, "Ship duplicate prevention");
}
