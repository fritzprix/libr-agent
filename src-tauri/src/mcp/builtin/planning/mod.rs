mod context;
mod goals;
mod todos;
mod tools;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::PlanningRepository;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use std::sync::Arc;

/// Planning MCP Server
///
/// Provides goal and todo management for agent sessions.
/// Session-scoped: each session gets dedicated planning state.
#[derive(Debug)]
pub struct PlanningServer {
    session_id: String,
    db: Arc<DatabaseConnection>,
}

impl PlanningServer {
    /// Create a new PlanningServer for the given session
    pub async fn new(session_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        let server = Self {
            session_id: session_id.clone(),
            db,
        };

        Ok(server)
    }

    /// Get tools statically (without an instance)
    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    /// Get metadata statically
    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Planning".to_string(),
            description: "Task planning and todo list management".to_string(),
            icon: None,
        }
    }
}

pub const NAME: &str = "planning";

#[async_trait]
impl BuiltinMCPServer for PlanningServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Session-scoped planning tools for goal and todo management"
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let target_session_id = _session_id.unwrap_or_else(|| self.session_id.clone());
        log::debug!(
            "Planning server tool called: {} for session: {}",
            tool_name,
            target_session_id
        );

        match tool_name {
            "createGoal" => goals::create_goal(self.db.as_ref(), &target_session_id, args).await,
            "updateGoal" => goals::update_goal(self.db.as_ref(), &target_session_id, args).await,
            "clearGoal" => goals::clear_goal(self.db.as_ref(), &target_session_id, args).await,
            "addTodo" => todos::add_todo(self.db.as_ref(), &target_session_id, args).await,
            "updateTodo" => todos::update_todo(self.db.as_ref(), &target_session_id, args).await,
            "clearSession" => {
                let repo = crate::state::get_planning_repository();
                match repo.clear_session(&target_session_id).await {
                    Ok(_) => Ok(MCPResult::success("✓ Session planning state cleared")),
                    Err(e) => Ok(guided_error(
                        ErrorCategory::DatabaseError,
                        format!("Failed to clear session: {}", e),
                        ToolGroup::Planning,
                    )
                    .with_guidance(vec!["Try again".to_string()])
                    .to_mcp_result()),
                }
            }
            "getCurrentState" => {
                // Extract optional include_checked param (defaults to true)
                let include_checked = args
                    .get("include_checked")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let context = context::get_service_context(
                    self.db.as_ref(),
                    &target_session_id,
                    include_checked,
                )
                .await;
                Ok(MCPResult::success_with_data(
                    &context.context_prompt,
                    context.structured_state.clone().unwrap_or(json!({})),
                ))
            }
            "reflect" => {
                let critique = match args.get("critique").and_then(|v| v.as_str()) {
                    Some(v) => v.to_string(),
                    None => return Ok(missing_param_error("critique", ToolGroup::Planning)),
                };
                let reflection = match args.get("reflection").and_then(|v| v.as_str()) {
                    Some(v) => v.to_string(),
                    None => return Ok(missing_param_error("reflection", ToolGroup::Planning)),
                };
                let next_action = match args.get("nextAction").and_then(|v| v.as_str()) {
                    Some(v) => v.to_string(),
                    None => return Ok(missing_param_error("nextAction", ToolGroup::Planning)),
                };
                let message = format!(
                    "## Reflection & Critique\n\n**Critique:**\n{}\n\n**Reflection:**\n{}\n\n**Next Action:**\n{}\n\n> Based on this reflection, proceed with the \"Next Action\" carefully.",
                    critique, reflection, next_action
                );
                let hint =
                    SuccessHint::new(message, vec![format!("Proceed with: {}", next_action)]);
                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "critique": critique,
                    "reflection": reflection,
                    "nextAction": next_action
                }))))
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Service context always includes completed todos for full planning visibility
        context::get_service_context(self.db.as_ref(), &self.session_id, true).await
    }
}
