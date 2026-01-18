use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext};
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

mod operations;
mod queries;

use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
struct ContextCache {
    prompt: String,
    last_update: Instant,
}

/// Assistant MCP Server
///
/// Provides global assistant configuration management.
/// Global scope: Assistants are shared across all sessions (no session_id FK).
#[derive(Debug)]
pub struct AssistantServer {
    db: Arc<DatabaseConnection>,
    cache: Arc<RwLock<Option<ContextCache>>>,
}

impl AssistantServer {
    /// Create a new AssistantServer
    ///
    /// Note: Unlike other servers, this is NOT session-bound.
    /// Assistants are global and can be reused across multiple sessions.
    /// Assistants are global and can be reused across multiple sessions.
    pub async fn new(db: Arc<DatabaseConnection>) -> Result<Self, String> {
        let server = Self {
            db,
            cache: Arc::new(RwLock::new(None)),
        };
        Ok(server)
    }

    pub fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub(crate) async fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.try_write() {
            *cache = None;
        }
    }

    pub fn tools_static() -> Vec<MCPTool> {
        vec![
            create_create_assistant_tool(),
            create_update_assistant_tool(),
            create_delete_assistant_tool(),
            create_list_assistants_tool(),
            create_get_assistant_tool(),
            create_search_assistant_tool(),
        ]
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
        Self::tools_static()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        log::debug!("Assistant server tool called: {}", tool_name);

        let db = self.get_db();

        match tool_name {
            "createAssistant" => operations::create_assistant(self, args).await,
            "updateAssistant" => operations::update_assistant(self, args).await,
            "deleteAssistant" => operations::delete_assistant(self, args).await,
            "listAssistants" => queries::list_assistants(db, args).await,
            "getAssistant" => queries::get_assistant(db, args).await,
            "searchAssistant" => queries::search_assistant(db, args).await,
            _ => Err(format!(
                "Unknown tool: {}. Available tools: createAssistant, updateAssistant, deleteAssistant, listAssistants, getAssistant, searchAssistant",
                tool_name
            )),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        const CACHE_TTL: Duration = Duration::from_secs(5);

        if let Some(cache) = self.cache.read().await.as_ref() {
            if cache.last_update.elapsed() < CACHE_TTL {
                return ServiceContext {
                    context_prompt: cache.prompt.clone(),
                    structured_state: None,
                };
            }
        }

        use crate::entity::assistant::Entity as AssistantEntity;
        use sea_orm::{EntityTrait, PaginatorTrait};

        let total_count = AssistantEntity::find()
            .count(self.get_db())
            .await
            .unwrap_or(0);

        let context_prompt = format!(
            "# Assistant Server Status\n\
            **Scope**: Global (shared across all sessions)\n\
            **Status**: Active\n\
            **Active Assistants**: {}\n\
            **Features**: Create, update, delete, and manage assistant configurations",
            total_count
        );

        if let Ok(mut cache) = self.cache.try_write() {
            *cache = Some(ContextCache {
                prompt: context_prompt.clone(),
                last_update: Instant::now(),
            });
        }

        ServiceContext {
            context_prompt,
            structured_state: None,
        }
    }
}

/// Create the createAssistant tool definition
fn create_create_assistant_tool() -> MCPTool {
    MCPTool {
        name: "createAssistant".to_string(),
        title: Some("Create Assistant".to_string()),
        description: "Create a new global assistant configuration.

⚠️ CRITICAL WORKFLOW (MUST FOLLOW):
1. ALWAYS call listAssistants FIRST to check for duplicates
2. Verify 'name' is unique
3. Then call this tool to create

❌ NEVER create without checking for duplicates first".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Assistant name (Must be unique)" },
                "systemPrompt": { "type": "string", "description": "System prompt for the assistant" },
                "modelProvider": { "type": "string", "description": "AI model provider (e.g., openai, anthropic, ollama)" },
                "modelName": { "type": "string", "description": "Specific model name (e.g., gpt-4, claude-3-5-sonnet)" },
                "temperature": { "type": "number", "description": "Model temperature (0.0 to 1.0)" },
                "maxTokens": { "type": "integer", "description": "Maximum tokens for response" },
                "allowedBuiltInServiceAliases": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of allowed built-in service aliases (e.g., 'mcp_manager', 'workspace', 'browser')"
                },
                "mcpServerIds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of enabled MCP server IDs (must exist in mcp_servers table).\n\n⚠️ CRITICAL: IDs must be valid.\n1. Call builtin_mcp_manager__listMcpServers FIRST to get valid IDs\n2. Extract exact ID values from listMcpServers response\n3. Invalid IDs will cause validation error"
                }
            },
            "required": ["name"]
        })).unwrap(),
        annotations: None,
        output_schema: None,
    }
}

/// Create the updateAssistant tool definition
fn create_update_assistant_tool() -> MCPTool {
    MCPTool {
        name: "updateAssistant".to_string(),
        title: Some("Update Assistant".to_string()),
        description: "Update an existing assistant configuration.

⚠️ CRITICAL WORKFLOW:
1. Call getAssistant(id) FIRST to get current config
2. Extract exact 'id' from response
3. Include ONLY fields you want to change
4. Update 'allowedBuiltInServiceAliases' to enable/disable builtin tools".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "⚠️ Exact Assistant ID from getAssistant response" },
                "name": { "type": "string", "description": "New name" },
                "systemPrompt": { "type": "string", "description": "New system prompt" },
                "modelProvider": { "type": "string", "description": "New AI model provider" },
                "modelName": { "type": "string", "description": "New model name" },
                "temperature": { "type": "number", "description": "New temperature" },
                "maxTokens": { "type": "integer", "description": "New max tokens" },
                "allowedBuiltInServiceAliases": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Update list of allowed built-in service aliases"
                },
                "mcpServerIds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Update list of enabled MCP server IDs (must exist in mcp_servers table).\n\n⚠️ Use builtin_mcp_manager__listMcpServers to get valid IDs before updating"
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
        string_prop_required("⚠️ Exact Assistant ID from listAssistants/getAssistant response"),
    );

    MCPTool {
        name: "deleteAssistant".to_string(),
        title: Some("Delete Assistant".to_string()),
        description: "Delete an assistant configuration.

⚠️ WARNING: This action is permanent.
✅ ALWAYS verify the ID with getAssistant before deleting"
            .to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the listAssistants tool definition
fn create_list_assistants_tool() -> MCPTool {
    MCPTool {
        name: "listAssistants".to_string(),
        title: Some("List Assistants".to_string()),
        description: "List available assistants with pagination.

Returns 'id', 'name', and 'config' for each assistant.
Use 'limit' and 'offset' to navigate through results.".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "description": "Items to return (max 100)", "default": 20 },
                "offset": { "type": "integer", "description": "Items to skip", "default": 0 },
                "search": { "type": "string", "description": "Search term for filtering" }
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
        string_prop_required("⚠️ Exact Assistant ID from listAssistants response"),
    );

    MCPTool {
        name: "getAssistant".to_string(),
        title: Some("Get Assistant".to_string()),
        description: "Get full details of a specific assistant.

✅ Use this to retrieve the current configuration before updating."
            .to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}

/// Create the searchAssistant tool definition
fn create_search_assistant_tool() -> MCPTool {
    MCPTool {
        name: "searchAssistant".to_string(),
        title: Some("Search Assistant".to_string()),
        description: "Search assistants by name or configuration content.".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query text" },
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
    use crate::entity::assistant::Entity as AssistantEntity;
    use sea_orm::{ConnectionTrait, Database, Schema};

    async fn create_test_db() -> Arc<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory database");

        let schema = Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(AssistantEntity);

        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create table");

        Arc::new(db)
    }

    #[tokio::test]
    async fn test_create_and_get_assistant() {
        let db = create_test_db().await;
        // Use operations directly for specialized testing, or server.call_tool for integration
        let server = AssistantServer::new(db.clone())
            .await
            .expect("Failed to create server");

        // Create assistant
        let create_result = server
            .call_tool(
                "createAssistant",
                json!({
                    "name": "Test Assistant",
                    "systemPrompt": "You are a helpful assistant",
                    "modelProvider": "openai",
                    "modelName": "gpt-4",
                    "temperature": 0.7
                }),
                None,
            )
            .await
            .expect("Failed to create assistant");

        assert!(create_result.is_error == Some(false));

        // Extract the system-generated ID from the response
        let created_id = create_result
            .structured_content
            .as_ref()
            .and_then(|c| c.get("id"))
            .and_then(|id| id.as_str())
            .expect("Expected id in create response");

        // Get assistant using system-generated ID
        let get_result = server
            .call_tool("getAssistant", json!({"id": created_id}), None)
            .await
            .expect("Failed to get assistant");

        assert!(get_result.is_error == Some(false));
        // Note: structured_content is Option<Value>
        let content = get_result.structured_content.unwrap();
        assert_eq!(content["name"], "Test Assistant");
        assert_eq!(content["config"]["modelName"], "gpt-4");

        // Verify system-generated ID is returned and is not empty
        assert!(
            !created_id.is_empty(),
            "Expected non-empty system-generated ID"
        );
        assert!(
            created_id.len() > 10,
            "Expected CUID-like ID format (length > 10)"
        );
    }
}
