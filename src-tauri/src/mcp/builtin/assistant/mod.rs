use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;
use async_trait::async_trait;
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
}

impl AssistantServer {
    /// Create a new AssistantServer
    ///
    /// Note: Unlike other servers, this is NOT session-bound.
    /// Assistants are global and can be reused across multiple sessions.
    pub async fn new(db_pool: Arc<SqlitePool>) -> Result<Self, String> {
        let server = Self { db_pool };

        // Initialize database tables
        server.init_tables().await?;

        Ok(server)
    }

    /// Initialize database tables and indexes
    async fn init_tables(&self) -> Result<(), String> {
        // Create assistants table (global scope - no session_id FK)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS assistants (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                config TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create assistants table: {}", e))?;

        // Create indexes
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_assistants_updated ON assistants(updated_at DESC)",
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create index: {}", e))?;

        log::debug!("Assistant server tables initialized");

        Ok(())
    }

    /// Create a new assistant
    async fn create_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'name' parameter".to_string())?;

        // Extract config fields
        let mut config = args.get("config").cloned().unwrap_or(json!({}));

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
        if let Some(v) = args.get("tools") {
            config["tools"] = v.clone();
        }
        if let Some(v) = args.get("mcpServers") {
            config["mcpServers"] = v.clone();
        }

        // Validate config is a valid JSON object
        let config_str =
            serde_json::to_string(&config).map_err(|e| format!("Invalid config JSON: {}", e))?;

        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            INSERT INTO assistants (id, name, config, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(&config_str)
        .bind(now)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(_) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Assistant '{}' created successfully", name),
                }]),
                structured_content: Some(json!({
                    "success": true,
                    "id": id,
                    "name": name,
                    "config": config
                })),
                is_error: Some(false),
            }),
            Err(e) => {
                if e.to_string().contains("UNIQUE constraint failed") {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Assistant with id '{}' already exists", id),
                        }]),
                        structured_content: None,
                        is_error: Some(true),
                    })
                } else {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Failed to create assistant: {}", e),
                        }]),
                        structured_content: None,
                        is_error: Some(true),
                    })
                }
            }
        }
    }

    /// Update an existing assistant
    async fn update_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

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
            return Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Assistant '{}' not found", id),
                }]),
                structured_content: None,
                is_error: Some(true),
            });
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
        if let Some(v) = args.get("tools") {
            config["tools"] = v.clone();
        }
        if let Some(v) = args.get("mcpServers") {
            config["mcpServers"] = v.clone();
        }

        let config_str =
            serde_json::to_string(&config).map_err(|e| format!("Invalid config JSON: {}", e))?;

        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            UPDATE assistants
            SET name = ?, config = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(&config_str)
        .bind(now)
        .bind(id)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                if query_result.rows_affected() > 0 {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Assistant '{}' updated successfully", id),
                        }]),
                        structured_content: Some(json!({
                            "success": true,
                            "id": id,
                            "name": name,
                            "config": config
                        })),
                        is_error: Some(false),
                    })
                } else {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Assistant '{}' not found", id),
                        }]),
                        structured_content: None,
                        is_error: Some(true),
                    })
                }
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to update assistant: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Delete an assistant
    async fn delete_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

        let result = sqlx::query(
            r#"
            DELETE FROM assistants
            WHERE id = ?
            "#,
        )
        .bind(id)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                if query_result.rows_affected() > 0 {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Assistant '{}' deleted successfully", id),
                        }]),
                        structured_content: Some(json!({
                            "success": true,
                            "id": id
                        })),
                        is_error: Some(false),
                    })
                } else {
                    Ok(MCPResult {
                        content: Some(vec![MCPContent::Text {
                            text: format!("Assistant '{}' not found", id),
                        }]),
                        structured_content: None,
                        is_error: Some(true),
                    })
                }
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to delete assistant: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// List all assistants with pagination support
    async fn list_assistants(&self, args: Value) -> Result<MCPResult, String> {
        // Extract pagination parameters
        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(50)
            .min(100) as i32; // Default 50, max 100
        let offset = args.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        // Get total count for pagination metadata
        let total_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assistants")
            .fetch_one(self.db_pool.as_ref())
            .await
            .unwrap_or(0);

        // Fetch paginated results
        let result = sqlx::query_as::<_, (String, String, String, i64, i64)>(
            r#"
            SELECT id, name, config, created_at, updated_at
            FROM assistants
            ORDER BY updated_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db_pool.as_ref())
        .await;

        match result {
            Ok(rows) => {
                let assistants: Vec<Value> = rows
                    .into_iter()
                    .map(|(id, name, config_str, created_at, updated_at)| {
                        // Parse config JSON
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

                let has_more = (offset + limit) < total_count as i32;

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!(
                            "Found {} of {} assistants (showing {} to {})",
                            total_count,
                            total_count,
                            offset + 1,
                            (offset + assistants.len() as i32).min(total_count as i32)
                        ),
                    }]),
                    structured_content: Some(json!({
                        "assistants": assistants,
                        "total": total_count,
                        "limit": limit,
                        "offset": offset,
                        "returned": assistants.len(),
                        "has_more": has_more
                    })),
                    is_error: Some(false),
                })
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to list assistants: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Search assistants
    async fn search_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'query' parameter".to_string())?;

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

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Found {} assistants", assistants.len()),
                    }]),
                    structured_content: Some(json!({
                        "assistants": assistants,
                        "count": assistants.len()
                    })),
                    is_error: Some(false),
                })
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to search assistants: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Get an assistant by ID
    async fn get_assistant(&self, args: Value) -> Result<MCPResult, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'id' parameter".to_string())?;

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

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Assistant: {}", name),
                    }]),
                    structured_content: Some(json!({
                        "id": id,
                        "name": name,
                        "config": config,
                        "created_at": created_at,
                        "updated_at": updated_at
                    })),
                    is_error: Some(false),
                })
            }
            Ok(None) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Assistant '{}' not found", id),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to get assistant: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
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
                "tools": { 
                    "type": "array", 
                    "items": { "type": "string" },
                    "description": "List of enabled tool names"
                },
                "mcpServers": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of enabled MCP server names"
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
                "tools": { 
                    "type": "array", 
                    "items": { "type": "string" },
                    "description": "List of enabled tool names"
                },
                "mcpServers": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of enabled MCP server names"
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
    let mut props = HashMap::new();
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(100),
            Some("Maximum number of assistants to return (default: 50, max: 100)"),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of assistants to skip (default: 0)"),
        ),
    );

    MCPTool {
        name: "builtin_assistant__listAssistants".to_string(),
        title: Some("List Assistants".to_string()),
        description: "List all global assistant configurations with pagination support".to_string(),
        input_schema: object_schema(props, vec![]), // Both parameters are optional
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
        assert_eq!(structured_default["limit"], 50); // Default limit
        assert_eq!(structured_default["offset"], 0); // Default offset
        assert_eq!(structured_default["returned"], 25);
        assert_eq!(structured_default["has_more"], false);

        // Test limit exceeding max (should cap at 100)
        let capped = server
            .list_assistants(json!({"limit": 150}))
            .await
            .expect("Failed with oversized limit");

        let structured_capped = capped.structured_content.unwrap();
        assert_eq!(structured_capped["limit"], 100); // Capped at max
    }
}
