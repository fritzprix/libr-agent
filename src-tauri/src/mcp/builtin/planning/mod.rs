mod context;
mod goals;
mod scratchpad;
mod todos;
mod tools;

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
/// Provides goal/todo/scratchpad management for agent sessions.
/// Session-scoped: Each session gets dedicated planning state.
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

#[async_trait]
impl BuiltinMCPServer for PlanningServer {
    fn name(&self) -> &str {
        "planning"
    }

    fn description(&self) -> &str {
        "Session-scoped planning tools for goal/todo/scratchpad management"
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
            "createGoal" | "builtin_planning__createGoal" => {
                goals::create_goal(self.db.as_ref(), &target_session_id, args).await
            }
            "updateGoal" | "builtin_planning__updateGoal" => {
                goals::update_goal(self.db.as_ref(), &target_session_id, args).await
            }
            "clearGoal" | "builtin_planning__clearGoal" => {
                goals::clear_goal(self.db.as_ref(), &target_session_id, args).await
            }
            "addTodo" | "builtin_planning__addTodo" => {
                todos::add_todo(self.db.as_ref(), &target_session_id, args).await
            }
            "checkTodo" | "builtin_planning__checkTodo" => {
                todos::check_todo(self.db.as_ref(), &target_session_id, args).await
            }
            "cancelTodo" | "builtin_planning__cancelTodo" => {
                todos::cancel_todo(self.db.as_ref(), &target_session_id, args).await
            }
            "clearSession" | "builtin_planning__clearSession" => {
                let repo = crate::state::get_planning_repository();
                repo.clear_session(&target_session_id)
                    .await
                    .map(|_| MCPResult::success("✓ Session planning state cleared"))
                    .map_err(|e| format!("Failed to clear session: {}", e))
            }
            "addScratchpad" | "builtin_planning__addScratchpad" => {
                scratchpad::add_scratchpad(self.db.as_ref(), &target_session_id, args).await
            }
            "updateScratchpad" | "builtin_planning__updateScratchpad" => {
                scratchpad::update_scratchpad(self.db.as_ref(), &target_session_id, args).await
            }
            "listScratchpad" | "builtin_planning__listScratchpad" => {
                scratchpad::list_scratchpad(self.db.as_ref(), &target_session_id, args).await
            }
            "readScratchpad" | "builtin_planning__readScratchpad" => {
                scratchpad::read_scratchpad(self.db.as_ref(), &target_session_id, args).await
            }
            "clearScratchpad" | "builtin_planning__clearScratchpad" => {
                scratchpad::clear_scratchpad(self.db.as_ref(), &target_session_id, args).await
            }
            "getCurrentState" | "builtin_planning__getCurrentState" => {
                // Reuse get_service_context but return as tool result
                let context =
                    context::get_service_context(self.db.as_ref(), &target_session_id).await;
                Ok(MCPResult::success_with_data(
                    &context.context_prompt,
                    context.structured_state.clone().unwrap_or(json!({})),
                ))
            }
            "pauseAndThink" | "builtin_planning__pauseAndThink" => {
                scratchpad::pause_and_think(args).await
            }
            "critiqueAndReflection" | "builtin_planning__critiqueAndReflection" => {
                scratchpad::critique_and_reflection(args).await
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        context::get_service_context(self.db.as_ref(), &self.session_id).await
    }
}
