mod common;

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::entity::assistant::Model as AssistantModel;
use tauri_mcp_agent_lib::mcp::builtin::assistant::{operations, AssistantServer};
use tauri_mcp_agent_lib::mcp::builtin::planning::PlanningServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::repositories::{AssistantRepository, DbError, SqlitePlanningRepository};
use tauri_mcp_agent_lib::services::assistant_service::AssistantService;
use tauri_mcp_agent_lib::set_planning_repository;

struct RecordingAssistantRepository;

#[async_trait]
impl AssistantRepository for RecordingAssistantRepository {
    async fn create_assistant(
        &self,
        id: String,
        name: String,
        config: String,
    ) -> Result<AssistantModel, DbError> {
        Ok(AssistantModel {
            id,
            name,
            config,
            created_at: 0,
            updated_at: 0,
        })
    }

    async fn get_assistant(&self, _id: &str) -> Result<Option<AssistantModel>, DbError> {
        Ok(None)
    }

    async fn update_assistant(
        &self,
        id: &str,
        name: Option<String>,
        config: Option<String>,
    ) -> Result<AssistantModel, DbError> {
        Ok(AssistantModel {
            id: id.to_string(),
            name: name.unwrap_or_else(|| "existing".to_string()),
            config: config.unwrap_or_else(|| "{}".to_string()),
            created_at: 0,
            updated_at: 0,
        })
    }

    async fn delete_assistant(&self, _id: &str) -> Result<(), DbError> {
        Ok(())
    }

    async fn list_assistants(&self) -> Result<Vec<AssistantModel>, DbError> {
        Ok(vec![])
    }

    async fn list_assistants_paginated(
        &self,
        _limit: u64,
        _offset: u64,
    ) -> Result<Vec<AssistantModel>, DbError> {
        Ok(vec![])
    }

    async fn search_assistants(&self, _query: &str) -> Result<Vec<AssistantModel>, DbError> {
        Ok(vec![])
    }

    async fn check_assistant_exists(&self, _name: &str) -> Result<bool, DbError> {
        Ok(false)
    }

    async fn count_assistants(&self) -> Result<u64, DbError> {
        Ok(0)
    }
}

fn extract_text(result: &MCPResult) -> String {
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
async fn assistant_service_rejects_blank_names() {
    let repo = RecordingAssistantRepository;
    let result = AssistantService::create_assistant(
        &repo,
        "assistant-1".to_string(),
        "   ".to_string(),
        json!({}),
    )
    .await;

    let error = result.expect_err("blank assistant names should be rejected");
    assert!(error.contains("cannot be blank"));
}

#[tokio::test]
async fn assistant_builtin_rejects_blank_names() {
    let db = common::setup_test_db_with_migrations().await;
    let server = AssistantServer::new(Arc::new(db))
        .await
        .expect("assistant server should initialize");

    let result = operations::create_assistant(
        &server,
        json!({
            "name": "   ",
            "description": "ignored"
        }),
    )
    .await
    .expect("assistant creation should return an MCP result");

    assert_eq!(result.is_error, Some(true));
    assert!(extract_text(&result).contains("Assistant name cannot be blank"));
}

#[tokio::test]
async fn planning_server_rejects_blank_todo_descriptions() {
    let db = common::setup_test_db_with_migrations().await;
    set_planning_repository(SqlitePlanningRepository::new(db.clone()));

    let server = PlanningServer::new("planning-validation".to_string(), Arc::new(db))
        .await
        .expect("planning server should initialize");

    let result = server
        .call_tool(
            "addTodo",
            json!({
                "description": "   "
            }),
            None,
        )
        .await
        .expect("addTodo should return an MCP result");

    assert_eq!(result.is_error, Some(true));
    assert!(extract_text(&result).contains("Todo description cannot be blank"));
}
