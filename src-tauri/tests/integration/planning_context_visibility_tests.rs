use crate::common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::planning::PlanningServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
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

#[tokio::test]
async fn planning_context_surfaces_read_failures_instead_of_empty_state() {
    let db = common::setup_test_db().await;
    set_planning_repository(SqlitePlanningRepository::new(db.clone()));

    let server = PlanningServer::new("planning-context".to_string(), Arc::new(db))
        .await
        .expect("planning server should initialize");

    let result = server
        .call_tool("getCurrentState", json!({ "include_checked": true }), None)
        .await
        .expect("getCurrentState should return an MCP result");

    let text = extract_text(&result);

    assert!(
        text.contains("Current Goal: unavailable"),
        "goal read failures should not be reported as an empty goal: {text}"
    );
    assert!(
        text.contains("Tasks: unavailable"),
        "todo read failures should not be reported as an empty task list: {text}"
    );
    assert!(
        text.contains("Use getCurrentState to reload the current goal state")
            || text.contains("Use getCurrentState to load the current goal state"),
        "goal reload guidance should be visible: {text}"
    );
    assert!(
        !text.contains("No goal set") && !text.contains("Tasks: None"),
        "empty-state wording should not appear when reads failed: {text}"
    );
}
