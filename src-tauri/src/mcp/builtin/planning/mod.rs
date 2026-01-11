mod context;
mod goals;
mod scratchpad;
mod todos;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::MCPTool;
use async_trait::async_trait;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
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
        vec![
            MCPTool {
                name: "createGoal".to_string(),
                title: Some("Create Goal".to_string()),
                description: "Create a single goal for the session. Use when starting a new or complex task.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "goal": { "type": "string", "description": "The goal text to set for the session (e.g., \"Complete project setup\")." } },
                    "required": ["goal"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "updateGoal".to_string(),
                title: Some("Update Goal".to_string()),
                description: "Update the current goal. Use when the goal needs refinement or correction without clearing context.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "goal": { "type": "string", "description": "The new goal text." } },
                    "required": ["goal"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearGoal".to_string(),
                title: Some("Clear Goal".to_string()),
                description: "Clear the current goal. Use when finishing or abandoning the current goal.".to_string(),
                input_schema: serde_json::from_value(json!({ "type": "object", "properties": {} }))
                    .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "addTodo".to_string(),
                title: Some("Add Todo".to_string()),
                description: "Add a todo item to the goal. Supports 1-level nesting: you can add subtasks inline or specify a parentId to create a child task. Use to break down a goal into actionable steps.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "description": { "type": "string", "description": "The task to be done." },
                        "priority": { "type": "string", "enum": ["low", "medium", "high"], "description": "The priority of the todo item." },
                        "parentId": { "type": "number", "description": "Parent todo ID to create a subtask. Only top-level todos (without parentId) can be parents. Maximum 1-level nesting." },
                        "subtasks": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "description": { "type": "string", "description": "Subtask description" },
                                    "priority": { "type": "string", "enum": ["low", "medium", "high"] }
                                },
                                "required": ["description"]
                            },
                            "description": "Array of subtasks to create with this todo. Only allowed when creating a top-level todo (no parentId)."
                        }
                    },
                    "required": ["description"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "checkTodo".to_string(),
                title: Some("Check Todo".to_string()),
                description: "Mark a todo item as checked (completed) or unchecked, optionally with a completion summary. You can specify either id (database ID) or index (0-based position in the list).".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "number", "minimum": 1, "description": "The database ID of the todo to update" },
                        "index": { "type": "number", "minimum": 0, "description": "The 0-based index position of the todo in the current list" },
                        "checked": { "type": "boolean", "description": "Whether to mark the todo as checked (true) or unchecked (false). Defaults to true." },
                        "summary": { "type": "string", "description": "Optional summary or completion note for the todo (e.g., \"Completed with PR #42\")." }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearTodos".to_string(),
                title: Some("Clear Todos".to_string()),
                description: "Clear specific todos by their indices (0-based) or IDs. If no indices or IDs are provided, all todos will be cleared. Use to remove completed tasks or reset the todo list.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "number", "minimum": 1 }, "description": "Array of todo IDs to clear." },
                        "indices": { "type": "array", "items": { "type": "number", "minimum": 0 }, "description": "Array of todo indices (0-based) to clear." }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearSession".to_string(),
                title: Some("Clear Session".to_string()),
                description: "Clear all session state (goal, todos, and scratchpad items). Use to reset everything and start fresh.".to_string(),
                input_schema: serde_json::from_value(json!({ "type": "object", "properties": {} }))
                    .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "addScratchpad".to_string(),
                title: Some("Add Scratchpad".to_string()),
                description: "Add a note to your Scratchpad (Working Memory). Content here is ALWAYS visible in your context. Use this for keeping track of important findings, file paths, IDs, or intermediate analysis results that you need to reference frequently during the task.\n\nNOTE: The scratchpad has a strict limit of 10 items. If you reach this limit, you must use  to modify existing items or  to remove old ones before adding more.\n\nOptional source parameter: Provide the source of information for citation tracking (e.g., URLs, file paths, or tool result IDs like \"https://example.com/article\" or \"file://path/to/doc.txt\").".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "note": { "type": "string", "description": "The content to add to the scratchpad (e.g., \"User requested feature X\", \"File path: src/main.ts\")." },
                        "title": { "type": "string", "description": "Optional title for the note. Helps in identifying the note in the list." },
                        "source": { "type": "string", "description": "Optional source of the information for citation tracking. Examples: \"https://example.com/article\", \"file://workspace/docs/readme.md\", \"tool_result_id:abc123\"" },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Optional tags for categorization and filtering." }
                    },
                    "required": ["note"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "updateScratchpad".to_string(),
                title: Some("Update Scratchpad".to_string()),
                description: "Update an existing scratchpad note. Use this when you want to modify the content of a note (e.g., adding more findings, correcting information) identified by its title.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "The title of the scratchpad note to update." },
                        "note": { "type": "string", "description": "The new content for the note." },
                        "newTitle": { "type": "string", "description": "Optional: New title for the note if you want to rename it." }
                    },
                    "required": ["title", "note"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "listScratchpad".to_string(),
                title: Some("List Scratchpad".to_string()),
                description: "List scratchpad items with metadata (ID, title, tags) and content preview. Use this to find the IDs of items you want to read fully. Supports pagination and tag filtering.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "page": { "type": "number", "minimum": 1, "description": "Page number (default: 1)" },
                        "pageSize": { "type": "number", "minimum": 1, "description": "Items per page (default: 10)" },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Filter items by tags" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "readScratchpad".to_string(),
                title: Some("Read Scratchpad".to_string()),
                description: "Read the FULL content of specific scratchpad items by their IDs. You must provide the IDs of the items you want to read. Use listScratchpad first to find IDs.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "number", "minimum": 0 }, "description": "List of scratchpad IDs to read (Required)." }
                    },
                    "required": ["ids"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clearScratchpad".to_string(),
                title: Some("Clear Scratchpad".to_string()),
                description: "Remove a note from your Scratchpad. Use this to clear information that is no longer relevant to free up context window space.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": { "id": { "type": "number", "minimum": 0, "description": "The ID of the scratchpad item to clear." } },
                    "required": ["id"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "getCurrentState".to_string(),
                title: Some("Get Current State".to_string()),
                description: "Get current planning state including Goal, Todos, and Scratchpad as human-readable text. Use when you need detailed visibility into current planning state beyond what's shown in the system context.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "include_checked": { "type": "boolean", "description": "Whether to include checked todos in the output. Defaults to true." },
                        "include_scratchpad": { "type": "boolean", "description": "Whether to include scratchpad items in the output. Defaults to true." }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "pauseAndThink".to_string(),
                title: Some("Pause and Think".to_string()),
                description: "Pause to think about the problem, plan your approach, or analyze results before taking action. Use this when you need to reason through complex decisions or maintain context. Simpler alternative to sequentialthinking.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "thought": { "type": "string", "description": "Your current thought, analysis, or plan. Be clear and specific about what you are thinking through." },
                        "nextAction": { "type": "string", "description": "Optional: The specific next action you plan to take after this thought. Helps maintain continuity." }
                    },
                    "required": ["thought"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "critiqueAndReflection".to_string(),
                title: Some("Critique and Reflection".to_string()),
                description: "Reflect on the current state and provide a critique of the progress. Use this tool to pause, analyze what has been done, identify potential issues or missed steps, and plan the next actions carefully.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "critique": { "type": "string", "description": "A critical evaluation of the results achieved so far." },
                        "reflection": { "type": "string", "description": "Self-reflection on any shortcomings or areas for improvement in the process." },
                        "nextAction": { "type": "string", "description": "The expected next action based on the reflection." }
                    },
                    "required": ["critique", "reflection", "nextAction"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
        ]
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
            "clearTodos" | "builtin_planning__clearTodos" => {
                todos::clear_todos(self.db.as_ref(), &target_session_id, args).await
            }
            "clearSession" | "builtin_planning__clearSession" => {
                let txn = self
                    .db
                    .begin()
                    .await
                    .map_err(|e: sea_orm::DbErr| e.to_string())?;

                crate::entity::planning_goal::Entity::delete_many()
                    .filter(crate::entity::planning_goal::Column::SessionId.eq(&target_session_id))
                    .exec(&txn)
                    .await
                    .map_err(|e| e.to_string())?;

                crate::entity::planning_todo::Entity::delete_many()
                    .filter(crate::entity::planning_todo::Column::SessionId.eq(&target_session_id))
                    .exec(&txn)
                    .await
                    .map_err(|e| e.to_string())?;

                crate::entity::planning_scratchpad::Entity::delete_many()
                    .filter(
                        crate::entity::planning_scratchpad::Column::SessionId
                            .eq(&target_session_id),
                    )
                    .exec(&txn)
                    .await
                    .map_err(|e| e.to_string())?;

                txn.commit()
                    .await
                    .map_err(|e: sea_orm::DbErr| e.to_string())?;
                Ok(MCPResult::success("✓ Session planning state cleared"))
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

    async fn switch_context(&self, _options: ServiceContextOptions) -> Result<(), String> {
        Err("Context switching not supported for session-bound planning server".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        context::get_service_context(self.db.as_ref(), &self.session_id).await
    }
}
