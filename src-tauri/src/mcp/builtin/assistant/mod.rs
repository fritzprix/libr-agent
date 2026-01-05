use crate::entity::{assistant, assistant::Entity as AssistantEntity};
use crate::mcp::builtin::error_guidance::{
    duplicate_error, missing_param_error, not_found_error, operation_failed_error, SuccessHint,
    ToolGroup,
};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;
use async_trait::async_trait;
use sea_orm::*;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

/// Assistant MCP Server
///
/// Provides global assistant configuration management.
/// Global scope: Assistants are shared across all sessions (no session_id FK).
#[derive(Debug)]
pub struct AssistantServer {
    db_pool: Arc<SqlitePool>,
    db_conn: DatabaseConnection,
}

impl AssistantServer {
    /// Create a new AssistantServer
    ///
    /// Note: Unlike other servers, this is NOT session-bound.
    /// Assistants are global and can be reused across multiple sessions.
    pub async fn new(db_pool: Arc<SqlitePool>) -> Result<Self, String> {
        let db_conn = SqlxSqliteConnector::from_sqlx_sqlite_pool((*db_pool).clone());
        let server = Self { db_pool, db_conn };
        Ok(server)
    }

    fn get_db(&self) -> &DatabaseConnection {
        &self.db_conn
    }

    /// Create a new assistant
    async fn create_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let db = self.get_db();

        // Legacy support: generate ID if not provided
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(cuid2::create_id);

        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("name", ToolGroup::Assistant)),
        };

        // Extract config fields
        let mut config = args.get("config").cloned().unwrap_or(json!({}));

        // Map legacy/flat fields to config
        if let Some(v) = args.get("systemPrompt") {
            config["systemPrompt"] = v.clone();
        }
        if let Some(v) = args.get("description") {
            config["description"] = v.clone();
        }
        if let Some(v) = args.get("modelProvider") {
            config["modelProvider"] = v.clone();
        }
        if let Some(v) = args.get("modelName") {
            config["modelName"] = v.clone();
        }
        if let Some(v) = args.get("temperature") {
            config["temperature"] = v.clone();
        }
        if let Some(v) = args.get("maxTokens") {
            config["maxTokens"] = v.clone();
        }

        // Handle tools (v2) -> allowedBuiltInServiceAliases
        if let Some(v) = args.get("tools") {
            config["allowedBuiltInServiceAliases"] = v.clone();
        }
        // Handle allowedBuiltInServiceAliases (v1)
        if let Some(v) = args.get("allowedBuiltInServiceAliases") {
            config["allowedBuiltInServiceAliases"] = v.clone();
        }

        // Handle mcpServers (v2) and mcpServerIds (v1)
        if let Some(v) = args.get("mcpServers") {
            config["mcpServerIds"] = v.clone();
        } else if let Some(v) = args.get("mcpServerIds") {
            config["mcpServerIds"] = v.clone();
        }

        // Validate config is a valid JSON object
        let config_str =
            serde_json::to_string(&config).map_err(|e| format!("Invalid config JSON: {}", e))?;

        let now = chrono::Utc::now().timestamp_millis();

        let model = assistant::ActiveModel {
            id: Set(id.clone()),
            name: Set(name.to_string()),
            config: Set(config_str),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = AssistantEntity::insert(model).exec(db).await;

        match result {
            Ok(_) => {
                let hint = SuccessHint::new(
                    format!("Assistant '{}' created successfully", name),
                    vec![
                        "Use builtin_assistant__listAssistants to see all assistants".to_string(),
                        "Use builtin_assistant__updateAssistant to modify configuration"
                            .to_string(),
                    ],
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "success": true,
                    "id": id,
                    "name": name
                }))))
            }
            Err(e) => {
                if e.to_string().contains("UNIQUE constraint failed") {
                    Ok(duplicate_error("Assistant", &id, ToolGroup::Assistant))
                } else {
                    Ok(operation_failed_error(
                        "Create assistant",
                        &e.to_string(),
                        vec!["Check database connection".to_string()],
                        ToolGroup::Assistant,
                    ))
                }
            }
        }
    }

    /// Update an existing assistant
    async fn update_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("id", ToolGroup::Assistant)),
        };

        // Fetch existing assistant to merge config
        let existing = sqlx::query_as::<_, (String, String, String, i64, i64)>(
            "SELECT id, name, config, created_at, updated_at FROM assistants WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to fetch assistant: {}", e))?;

        let (mut name, mut config) = if let Some((_, n, c, _, _)) = existing {
            (n, serde_json::from_str::<Value>(&c).unwrap_or(json!({})))
        } else {
            return Ok(not_found_error("Assistant", id, ToolGroup::Assistant));
        };

        // Update name if provided
        if let Some(n) = args.get("name").and_then(|v| v.as_str()) {
            name = n.to_string();
        }

        // Update config from 'config' object if provided
        if let Some(c) = args.get("config").and_then(|v| v.as_object()) {
            for (k, v) in c {
                config[k] = v.clone();
            }
        }

        // Update config fields (individual overrides)
        if let Some(v) = args.get("systemPrompt") {
            config["systemPrompt"] = v.clone();
        }
        if let Some(v) = args.get("modelProvider") {
            config["modelProvider"] = v.clone();
        }
        if let Some(v) = args.get("modelName") {
            config["modelName"] = v.clone();
        }
        if let Some(v) = args.get("temperature") {
            config["temperature"] = v.clone();
        }
        if let Some(v) = args.get("maxTokens") {
            config["maxTokens"] = v.clone();
        }
        // Handle tools (v2) -> allowedBuiltInServiceAliases
        if let Some(v) = args.get("tools") {
            config["allowedBuiltInServiceAliases"] = v.clone();
        }
        // Handle allowedBuiltInServiceAliases (v1)
        if let Some(v) = args.get("allowedBuiltInServiceAliases") {
            config["allowedBuiltInServiceAliases"] = v.clone();
        }

        // Handle mcpServers (v2) and mcpServerIds (v1)
        if let Some(v) = args.get("mcpServers") {
            config["mcpServerIds"] = v.clone();
        }
        if let Some(v) = args.get("mcpServerIds") {
            config["mcpServerIds"] = v.clone();
        }

        let config_str =
            serde_json::to_string(&config).map_err(|e| format!("Invalid config JSON: {}", e))?;

        let now = chrono::Utc::now().timestamp_millis();
        let db = self.get_db();

        // Check if exists first
        let existing = AssistantEntity::find_by_id(id.to_string())
            .one(db)
            .await
            .map_err(|e| format!("Database query failed: {}", e))?;

        if existing.is_none() {
            return Ok(not_found_error("Assistant", id, ToolGroup::Assistant));
        }

        let model = assistant::ActiveModel {
            id: Set(id.to_string()),
            name: Set(name.to_string()),
            config: Set(config_str),
            created_at: NotSet,
            updated_at: Set(now),
        };

        let result = AssistantEntity::update(model).exec(db).await;

        match result {
            Ok(_) => {
                let hint = SuccessHint::new(
                    format!("Assistant '{}' updated successfully", id),
                    vec!["Use builtin_assistant__getAssistant to verify changes".to_string()],
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "success": true,
                    "id": id,
                    "name": name,
                    "config": config
                }))))
            }
            Err(e) => Ok(operation_failed_error(
                "Update assistant",
                &e.to_string(),
                vec![
                    "Verify the config JSON is valid".to_string(),
                    "Check database connectivity".to_string(),
                    "Use builtin_assistant__getAssistant to verify the assistant exists"
                        .to_string(),
                ],
                ToolGroup::Assistant,
            )),
        }
    }

    /// Delete an assistant
    async fn delete_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let db = self.get_db();

        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("id", ToolGroup::Assistant)),
        };

        let result = AssistantEntity::delete_by_id(id.to_string()).exec(db).await;

        match result {
            Ok(delete_result) => {
                if delete_result.rows_affected > 0 {
                    let hint = SuccessHint::new(
                        format!("Assistant '{}' deleted successfully", id),
                        vec![
                            "Use builtin_assistant__listAssistants to see remaining assistants"
                                .to_string(),
                        ],
                    );

                    Ok(hint.to_mcp_result_with_data(Some(json!({
                        "success": true,
                        "id": id
                    }))))
                } else {
                    Ok(not_found_error("Assistant", id, ToolGroup::Assistant))
                }
            }
            Err(e) => Ok(operation_failed_error(
                "Delete assistant",
                &e.to_string(),
                vec![
                    "Verify the assistant ID is correct".to_string(),
                    "Use builtin_assistant__listAssistants to see existing assistants".to_string(),
                    "Check database connectivity".to_string(),
                ],
                ToolGroup::Assistant,
            )),
        }
    }

    /// List all assistants with pagination support
    async fn list_assistants(&self, args: Value) -> Result<MCPResult, String> {
        let db = self.get_db();

        // Legacy support: page/pageSize -> limit/offset
        let page = args
            .get("page")
            .and_then(|v| v.as_i64())
            .unwrap_or(1)
            .max(1);
        let page_size = args
            .get("pageSize")
            .and_then(|v| v.as_i64())
            .unwrap_or(20)
            .clamp(1, 100);

        // Also support direct limit/offset if provided (v2 native)
        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(page_size)
            .clamp(1, 100);
        let offset = args
            .get("offset")
            .and_then(|v| v.as_i64())
            .unwrap_or((page - 1) * page_size);

        // Get total count for pagination metadata
        let total_count = AssistantEntity::find().count(db).await.unwrap_or(0) as i64;

        // Fetch paginated results
        let result = AssistantEntity::find()
            .order_by_desc(assistant::Column::UpdatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(db)
            .await;

        match result {
            Ok(models) => {
                let assistants: Vec<Value> = models
                    .into_iter()
                    .map(|model| {
                        // Parse config JSON
                        let config =
                            serde_json::from_str::<Value>(&model.config).unwrap_or(json!({}));

                        json!({
                            "id": model.id,
                            "name": model.name,
                            "config": config,
                            "created_at": model.created_at,
                            "updated_at": model.updated_at
                        })
                    })
                    .collect();

                let has_more = (offset + limit) < total_count;

                let hint = SuccessHint::new(
                    format!(
                        "Found {} of {} assistants (showing {} to {})",
                        total_count,
                        total_count,
                        offset + 1,
                        (offset + assistants.len() as i64).min(total_count)
                    ),
                    if has_more {
                        vec![format!(
                            "Use limit={} offset={} to see more assistants",
                            limit,
                            offset + limit
                        )]
                    } else if total_count > 0 {
                        vec!["Use builtin_assistant__getAssistant to view details".to_string()]
                    } else {
                        vec![
                            "Use builtin_assistant__createAssistant to create an assistant"
                                .to_string(),
                        ]
                    },
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "assistants": assistants,
                    "total": total_count,
                    "limit": limit,
                    "offset": offset,
                    "returned": assistants.len(),
                    "has_more": has_more
                }))))
            }
            Err(e) => Ok(operation_failed_error(
                "List assistants",
                &e.to_string(),
                vec![
                    "Check database connectivity".to_string(),
                    "Verify pagination parameters are valid integers".to_string(),
                ],
                ToolGroup::Assistant,
            )),
        }
    }

    /// Search assistants
    async fn search_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("query", ToolGroup::Assistant)),
        };

        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(10)
            .min(100);

        let search_pattern = format!("%{}%", query);

        let result = sqlx::query_as::<_, (String, String, String, i64, i64)>(
            r#"
            SELECT id, name, config, created_at, updated_at
            FROM assistants
            WHERE name LIKE ? OR config LIKE ?
            ORDER BY updated_at DESC
            LIMIT ?
            "#,
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(self.db_pool.as_ref())
        .await;

        match result {
            Ok(rows) => {
                let assistants: Vec<Value> = rows
                    .into_iter()
                    .map(|(id, name, config_str, created_at, updated_at)| {
                        let config =
                            serde_json::from_str::<Value>(&config_str).unwrap_or(json!({}));
                        json!({
                            "id": id,
                            "name": name,
                            "config": config,
                            "created_at": created_at,
                            "updated_at": updated_at
                        })
                    })
                    .collect();

                let hint = SuccessHint::new(
                    format!("Found {} assistants", assistants.len()),
                    if assistants.is_empty() {
                        vec![
                            format!("No assistants match '{}'", query),
                            "Use builtin_assistant__listAssistants to see all assistants"
                                .to_string(),
                        ]
                    } else {
                        vec!["Use builtin_assistant__getAssistant to view details".to_string()]
                    },
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "assistants": assistants,
                    "count": assistants.len()
                }))))
            }
            Err(e) => Ok(operation_failed_error(
                "Search assistants",
                &e.to_string(),
                vec![
                    "Check database connectivity".to_string(),
                    "Verify query parameter is a valid string".to_string(),
                ],
                ToolGroup::Assistant,
            )),
        }
    }

    /// Get an assistant by ID
    async fn get_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("id", ToolGroup::Assistant)),
        };

        let result = sqlx::query_as::<_, (String, String, String, i64, i64)>(
            r#"
            SELECT id, name, config, created_at, updated_at
            FROM assistants
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.db_pool.as_ref())
        .await;

        match result {
            Ok(Some((id, name, config_str, created_at, updated_at))) => {
                // Parse config JSON
                let config = serde_json::from_str::<Value>(&config_str).unwrap_or(json!({}));

                let hint = SuccessHint::new(
                    format!("Assistant: {}", name),
                    vec![
                        "Use builtin_assistant__updateAssistant to modify configuration"
                            .to_string(),
                        "Use builtin_assistant__deleteAssistant to remove this assistant"
                            .to_string(),
                    ],
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "id": id,
                    "name": name,
                    "config": config,
                    "created_at": created_at,
                    "updated_at": updated_at
                }))))
            }
            Ok(None) => Ok(not_found_error("Assistant", id, ToolGroup::Assistant)),
            Err(e) => Ok(operation_failed_error(
                "Get assistant",
                &e.to_string(),
                vec![
                    "Verify the assistant ID is correct".to_string(),
                    "Use builtin_assistant__listAssistants to see existing assistants".to_string(),
                    "Check database connectivity".to_string(),
                ],
                ToolGroup::Assistant,
            )),
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for AssistantServer {
    fn name(&self) -> &str {
        "assistant"
    }

    fn description(&self) -> &str {
        "Global assistant configuration management (shared across all sessions)"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            create_create_assistant_tool(),
            create_update_assistant_tool(),
            create_delete_assistant_tool(),
            create_list_assistants_tool(),
            create_get_assistant_tool(),
            create_search_assistant_tool(),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        log::debug!("Assistant server tool called: {}", tool_name);

        match tool_name {
            "createAssistant" | "builtin_assistant__createAssistant" => {
                self.create_assistant(args).await
            }
            "updateAssistant" | "builtin_assistant__updateAssistant" => {
                self.update_assistant(args).await
            }
            "deleteAssistant" | "builtin_assistant__deleteAssistant" => {
                self.delete_assistant(args).await
            }
            "listAssistants" | "builtin_assistant__listAssistants" => {
                self.list_assistants(args).await
            }
            "getAssistant" | "builtin_assistant__getAssistant" => {
                self.get_assistant(args).await
            }
            "searchAssistant" | "builtin_assistant__searchAssistant" => {
                self.search_assistant(args).await
            }
            _ => Err(format!(
                "Unknown tool: {}. Available tools: createAssistant, updateAssistant, deleteAssistant, listAssistants, getAssistant, searchAssistant",
                tool_name
            )),
        }
    }

    async fn switch_context(&self, _options: ServiceContextOptions) -> Result<(), String> {
        Err("Context switching not supported for global assistant server".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: "# Assistant Server Status\n\
                **Scope**: Global (shared across all sessions)\n\
                **Status**: Active\n\
                **Features**: Create, update, delete, and manage assistant configurations"
                .to_string(),
            structured_state: None,
        }
    }
}

/// Create the createAssistant tool definition
fn create_create_assistant_tool() -> MCPTool {
    MCPTool {
        name: "builtin_assistant__createAssistant".to_string(),
        title: Some("Create Assistant".to_string()),
        description: "Create a new global assistant configuration".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Unique assistant identifier" },
                "name": { "type": "string", "description": "Assistant name" },
                "systemPrompt": { "type": "string", "description": "System prompt for the assistant" },
                "modelProvider": { "type": "string", "description": "AI model provider (e.g., openai, anthropic)" },
                "modelName": { "type": "string", "description": "Specific model name (e.g., gpt-4)" },
                "temperature": { "type": "number", "description": "Model temperature (0.0 to 1.0)" },
                "maxTokens": { "type": "integer", "description": "Maximum tokens for response" },
                "allowedBuiltInServiceAliases": { 
                    "type": "array", 
                    "items": { "type": "string" },
                    "description": "List of allowed built-in service aliases"
                },
                "mcpServerIds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of enabled MCP server IDs"
                }
            },
            "required": ["id", "name"]
        })).unwrap(),
        annotations: None,
        output_schema: None,
    }
}

/// Create the updateAssistant tool definition
fn create_update_assistant_tool() -> MCPTool {
    MCPTool {
        name: "builtin_assistant__updateAssistant".to_string(),
        title: Some("Update Assistant".to_string()),
        description: "Update an existing assistant configuration".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Assistant identifier" },
                "name": { "type": "string", "description": "Assistant name" },
                "systemPrompt": { "type": "string", "description": "System prompt for the assistant" },
                "modelProvider": { "type": "string", "description": "AI model provider" },
                "modelName": { "type": "string", "description": "Specific model name" },
                "temperature": { "type": "number", "description": "Model temperature" },
                "maxTokens": { "type": "integer", "description": "Maximum tokens" },
                "allowedBuiltInServiceAliases": { 
                    "type": "array", 
                    "items": { "type": "string" },
                    "description": "List of allowed built-in service aliases"
                },
                "mcpServerIds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of enabled MCP server IDs"
                }
            },
            "required": ["id"]
        })).unwrap(),
        annotations: None,
        output_schema: None,
    }
}

/// Create the deleteAssistant tool definition
fn create_delete_assistant_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        string_prop_required("Assistant identifier"),
    );

    MCPTool {
        name: "builtin_assistant__deleteAssistant".to_string(),
        title: Some("Delete Assistant".to_string()),
        description: "Delete an assistant configuration".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the listAssistants tool definition
fn create_list_assistants_tool() -> MCPTool {
    MCPTool {
        name: "builtin_assistant__listAssistants".to_string(),
        title: Some("List Assistants".to_string()),
        description: "List available assistants with pagination".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "page": { "type": "integer", "description": "Page number (1-based)", "default": 1 },
                "pageSize": { "type": "integer", "description": "Items per page", "default": 20 },
                "search": { "type": "string", "description": "Search term for filtering assistants" }
            }
        })).unwrap(),
        annotations: None,
        output_schema: None,
    }
}

/// Create the getAssistant tool definition
fn create_get_assistant_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        string_prop_required("Assistant identifier"),
    );

    MCPTool {
        name: "builtin_assistant__getAssistant".to_string(),
        title: Some("Get Assistant".to_string()),
        description: "Get an assistant configuration by ID".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the searchAssistant tool definition
fn create_search_assistant_tool() -> MCPTool {
    MCPTool {
        name: "builtin_assistant__searchAssistant".to_string(),
        title: Some("Search Assistant".to_string()),
        description: "Search assistants by name or configuration content".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "limit": { "type": "integer", "description": "Maximum number of results" }
            },
            "required": ["query"]
        }))
        .unwrap(),
        annotations: None,
        output_schema: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn create_test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("Invalid database URL")
            .create_if_missing(true);

        SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .expect("Failed to create test pool")
    }

    #[tokio::test]
    async fn test_create_and_get_assistant() {
        let pool = Arc::new(create_test_pool().await);
        let server = AssistantServer::new(pool)
            .await
            .expect("Failed to create server");

        // Create assistant
        let create_result = server
            .create_assistant(json!({
                "id": "test-assistant",
                "name": "Test Assistant",
                "config": {
                    "model": "gpt-4",
                    "temperature": 0.7
                }
            }))
            .await
            .expect("Failed to create assistant");

        assert!(create_result.is_error == Some(false));

        // Get assistant
        let get_result = server
            .get_assistant(json!({"id": "test-assistant"}))
            .await
            .expect("Failed to get assistant");

        assert!(get_result.is_error == Some(false));
        let structured = get_result.structured_content.unwrap();
        assert_eq!(structured["name"], "Test Assistant");
        assert_eq!(structured["config"]["model"], "gpt-4");
    }

    #[tokio::test]
    async fn test_update_assistant() {
        let pool = Arc::new(create_test_pool().await);
        let server = AssistantServer::new(pool)
            .await
            .expect("Failed to create server");

        // Create assistant
        server
            .create_assistant(json!({
                "id": "update-test",
                "name": "Original Name",
                "config": {"version": 1}
            }))
            .await
            .expect("Failed to create assistant");

        // Update assistant
        let update_result = server
            .update_assistant(json!({
                "id": "update-test",
                "config": {"version": 2, "updated": true}
            }))
            .await
            .expect("Failed to update assistant");

        assert!(update_result.is_error == Some(false));

        // Verify update
        let get_result = server
            .get_assistant(json!({"id": "update-test"}))
            .await
            .expect("Failed to get assistant");

        let structured = get_result.structured_content.unwrap();
        assert_eq!(structured["config"]["version"], 2);
        assert_eq!(structured["config"]["updated"], true);
    }

    #[tokio::test]
    async fn test_list_and_delete_assistants() {
        let pool = Arc::new(create_test_pool().await);
        let server = AssistantServer::new(pool)
            .await
            .expect("Failed to create server");

        // Create multiple assistants
        server
            .create_assistant(json!({
                "id": "assistant-1",
                "name": "Assistant 1",
                "config": {}
            }))
            .await
            .expect("Failed to create assistant 1");

        server
            .create_assistant(json!({
                "id": "assistant-2",
                "name": "Assistant 2",
                "config": {}
            }))
            .await
            .expect("Failed to create assistant 2");

        // List assistants
        let list_result = server
            .list_assistants(json!({}))
            .await
            .expect("Failed to list assistants");

        assert!(list_result.is_error == Some(false));
        let structured = list_result.structured_content.unwrap();
        assert_eq!(structured["total"], 2);
        assert_eq!(structured["returned"], 2);

        // Delete one assistant
        let delete_result = server
            .delete_assistant(json!({"id": "assistant-1"}))
            .await
            .expect("Failed to delete assistant");

        assert!(delete_result.is_error == Some(false));

        // List again - should have 1 assistant
        let list_result2 = server
            .list_assistants(json!({}))
            .await
            .expect("Failed to list assistants");

        let structured2 = list_result2.structured_content.unwrap();
        assert_eq!(structured2["total"], 1);
        assert_eq!(structured2["returned"], 1);
    }

    #[tokio::test]
    async fn test_global_scope() {
        // This test verifies that AssistantServer is global scope
        // by showing that assistants persist across different "sessions"
        let pool = Arc::new(create_test_pool().await);

        // Create first server instance
        let server1 = AssistantServer::new(pool.clone())
            .await
            .expect("Failed to create server 1");

        // Create assistant
        server1
            .create_assistant(json!({
                "id": "global-assistant",
                "name": "Global Assistant",
                "config": {"shared": true}
            }))
            .await
            .expect("Failed to create assistant");

        // Create second server instance (simulating different session)
        let server2 = AssistantServer::new(pool)
            .await
            .expect("Failed to create server 2");

        // Get assistant from second instance - should work because it's global
        let get_result = server2
            .get_assistant(json!({"id": "global-assistant"}))
            .await
            .expect("Failed to get assistant from server 2");

        assert!(get_result.is_error == Some(false));
        let structured = get_result.structured_content.unwrap();
        assert_eq!(structured["name"], "Global Assistant");
        assert_eq!(structured["config"]["shared"], true);
    }

    #[tokio::test]
    async fn test_list_assistants_pagination() {
        let pool = Arc::new(create_test_pool().await);
        let server = AssistantServer::new(pool)
            .await
            .expect("Failed to create server");

        // Create 25 assistants
        for i in 1..=25 {
            server
                .create_assistant(json!({
                    "id": format!("assistant-{:02}", i),
                    "name": format!("Assistant {}", i),
                    "config": {"index": i}
                }))
                .await
                .unwrap_or_else(|_| panic!("Failed to create assistant {}", i));
        }

        // Test page 1: limit=10, offset=0
        let page1 = server
            .list_assistants(json!({"limit": 10, "offset": 0}))
            .await
            .expect("Failed to get page 1");

        assert_eq!(page1.is_error, Some(false));
        let structured1 = page1.structured_content.unwrap();
        assert_eq!(structured1["total"], 25);
        assert_eq!(structured1["limit"], 10);
        assert_eq!(structured1["offset"], 0);
        assert_eq!(structured1["returned"], 10);
        assert_eq!(structured1["has_more"], true);
        let assistants1 = structured1["assistants"].as_array().unwrap();
        assert_eq!(assistants1.len(), 10);

        // Test page 2: limit=10, offset=10
        let page2 = server
            .list_assistants(json!({"limit": 10, "offset": 10}))
            .await
            .expect("Failed to get page 2");

        let structured2 = page2.structured_content.unwrap();
        assert_eq!(structured2["total"], 25);
        assert_eq!(structured2["limit"], 10);
        assert_eq!(structured2["offset"], 10);
        assert_eq!(structured2["returned"], 10);
        assert_eq!(structured2["has_more"], true);
        let assistants2 = structured2["assistants"].as_array().unwrap();
        assert_eq!(assistants2.len(), 10);

        // Verify different assistants
        assert_ne!(assistants1[0]["id"], assistants2[0]["id"]);

        // Test page 3: limit=10, offset=20 (last page with 5 items)
        let page3 = server
            .list_assistants(json!({"limit": 10, "offset": 20}))
            .await
            .expect("Failed to get page 3");

        let structured3 = page3.structured_content.unwrap();
        assert_eq!(structured3["total"], 25);
        assert_eq!(structured3["limit"], 10);
        assert_eq!(structured3["offset"], 20);
        assert_eq!(structured3["returned"], 5);
        assert_eq!(structured3["has_more"], false);
        let assistants3 = structured3["assistants"].as_array().unwrap();
        assert_eq!(assistants3.len(), 5);

        // Test default pagination (no params)
        let default_page = server
            .list_assistants(json!({}))
            .await
            .expect("Failed to get default page");

        let structured_default = default_page.structured_content.unwrap();
        assert_eq!(structured_default["total"], 25);
        assert_eq!(structured_default["limit"], 20); // Default limit
        assert_eq!(structured_default["offset"], 0); // Default offset
        assert_eq!(structured_default["returned"], 20);
        assert_eq!(structured_default["has_more"], true);

        // Test limit exceeding max (should cap at 100)
        let capped = server
            .list_assistants(json!({"limit": 150}))
            .await
            .expect("Failed with oversized limit");

        let structured_capped = capped.structured_content.unwrap();
        assert_eq!(structured_capped["limit"], 100); // Capped at max
    }
}
