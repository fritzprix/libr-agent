use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::AssistantRepository;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

mod operations;
mod queries;
pub mod tools;

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
        match self.cache.try_write() {
            Ok(mut cache) => *cache = None,
            Err(_) => log::warn!("Failed to invalidate assistant cache - lock contention"),
        }
    }

    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    /// Get metadata statically
    pub fn metadata_static() -> crate::mcp::types::BuiltinServerMetadata {
        crate::mcp::types::BuiltinServerMetadata {
            display_name: "Assistant Manager".to_string(),
            description: "Global assistant configuration management (shared across all sessions)"
                .to_string(),
            icon: None,
        }
    }
}

pub const NAME: &str = "assistant";

#[async_trait]
impl BuiltinMCPServer for AssistantServer {
    fn name(&self) -> &str {
        NAME
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
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        log::debug!("Assistant server tool called: {}", tool_name);

        let db = self.get_db();

        match tool_name {
            "createAssistant" => operations::create_assistant(self, args).await,
            "updateAssistant" => operations::update_assistant(self, args, session_id).await,
            "deleteAssistant" => operations::delete_assistant(self, args, session_id).await,
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

        // Use repository from db connection
        let repo = crate::repositories::SqliteAssistantRepository::new(self.get_db().clone());
        let total_count = repo.count_assistants().await.unwrap_or(0);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::assistant::Entity as AssistantEntity;
    use crate::entity::mcp_server::Entity as MCPServerEntity;
    use sea_orm::{ConnectionTrait, Database, Schema};
    use serde_json::json;

    async fn create_test_db() -> Arc<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory database");

        let schema = Schema::new(db.get_database_backend());

        // Create assistants table
        let stmt = schema.create_table_from_entity(AssistantEntity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create assistants table");

        // Create mcp_servers table for validation tests
        let stmt = schema.create_table_from_entity(MCPServerEntity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create mcp_servers table");

        Arc::new(db)
    }

    async fn setup_test_server() -> AssistantServer {
        let db = create_test_db().await;
        AssistantServer::new(db)
            .await
            .expect("Failed to create server")
    }

    #[tokio::test]
    async fn test_create_and_get_assistant() {
        let server = setup_test_server().await;

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

    #[tokio::test]
    async fn test_update_assistant() {
        let server = setup_test_server().await;

        // Create assistant first
        let create_result = server
            .call_tool(
                "createAssistant",
                json!({
                    "name": "Original Name",
                    "modelProvider": "openai",
                    "modelName": "gpt-4"
                }),
                None,
            )
            .await
            .expect("Failed to create assistant");

        let created_id = create_result
            .structured_content
            .as_ref()
            .and_then(|c| c.get("id"))
            .and_then(|id| id.as_str())
            .expect("Expected id");

        // Update assistant
        let update_result = server
            .call_tool(
                "updateAssistant",
                json!({
                    "id": created_id,
                    "name": "Updated Name",
                    "temperature": 0.9
                }),
                None,
            )
            .await
            .expect("Failed to update assistant");

        assert!(update_result.is_error == Some(false));

        // Verify update
        let get_result = server
            .call_tool("getAssistant", json!({"id": created_id}), None)
            .await
            .expect("Failed to get assistant");

        let content = get_result.structured_content.unwrap();
        assert_eq!(content["name"], "Updated Name");
        assert_eq!(content["config"]["temperature"], 0.9);
        assert_eq!(content["config"]["modelName"], "gpt-4"); // Unchanged
    }

    #[tokio::test]
    async fn test_delete_assistant() {
        let server = setup_test_server().await;

        // Create assistant
        let create_result = server
            .call_tool("createAssistant", json!({"name": "To Delete"}), None)
            .await
            .expect("Failed to create assistant");

        let created_id = create_result
            .structured_content
            .as_ref()
            .and_then(|c| c.get("id"))
            .and_then(|id| id.as_str())
            .expect("Expected id");

        // Delete assistant
        let delete_result = server
            .call_tool("deleteAssistant", json!({"id": created_id}), None)
            .await
            .expect("Failed to delete assistant");

        assert!(delete_result.is_error == Some(false));

        // Verify deletion
        let get_result = server
            .call_tool("getAssistant", json!({"id": created_id}), None)
            .await
            .expect("Failed to get assistant");

        assert!(get_result.is_error == Some(true)); // Should be not found error
    }

    #[tokio::test]
    async fn test_list_assistants_pagination() {
        let server = setup_test_server().await;

        // Create multiple assistants
        for i in 1..=5 {
            server
                .call_tool(
                    "createAssistant",
                    json!({"name": format!("Assistant {}", i)}),
                    None,
                )
                .await
                .expect("Failed to create assistant");
        }

        // Test first page
        let list_result = server
            .call_tool("listAssistants", json!({"limit": 2, "offset": 0}), None)
            .await
            .expect("Failed to list assistants");

        let content = list_result.structured_content.unwrap();
        assert_eq!(content["returned"], 2);
        assert_eq!(content["total"], 5);
        assert_eq!(content["has_more"], true);

        // Test second page
        let list_result = server
            .call_tool("listAssistants", json!({"limit": 2, "offset": 2}), None)
            .await
            .expect("Failed to list assistants");

        let content = list_result.structured_content.unwrap();
        assert_eq!(content["returned"], 2);
        assert_eq!(content["has_more"], true);
    }

    #[tokio::test]
    async fn test_duplicate_name_error() {
        let server = setup_test_server().await;

        // Create first assistant
        server
            .call_tool("createAssistant", json!({"name": "Duplicate Name"}), None)
            .await
            .expect("Failed to create assistant");

        // Try to create duplicate
        let duplicate_result = server
            .call_tool("createAssistant", json!({"name": "Duplicate Name"}), None)
            .await
            .expect("Failed to call createAssistant");

        assert!(duplicate_result.is_error == Some(true));
    }

    #[tokio::test]
    async fn test_search_assistant() {
        let server = setup_test_server().await;

        // Create assistants with different names
        server
            .call_tool("createAssistant", json!({"name": "Code Helper"}), None)
            .await
            .expect("Failed to create assistant");

        server
            .call_tool(
                "createAssistant",
                json!({"name": "Writing Assistant"}),
                None,
            )
            .await
            .expect("Failed to create assistant");

        // Search for "Code"
        let search_result = server
            .call_tool("searchAssistant", json!({"query": "Code"}), None)
            .await
            .expect("Failed to search assistants");

        let content = search_result.structured_content.unwrap();
        let assistants = content["assistants"].as_array().unwrap();
        assert_eq!(assistants.len(), 1);
        assert_eq!(assistants[0]["name"], "Code Helper");
    }

    #[tokio::test]
    async fn test_invalid_mcp_server_ids() {
        let server = setup_test_server().await;

        // Try to create with invalid MCP server IDs
        let result = server
            .call_tool(
                "createAssistant",
                json!({
                    "name": "Test",
                    "mcpServerIds": ["nonexistent-server"]
                }),
                None,
            )
            .await
            .expect("Failed to call createAssistant");

        assert!(result.is_error == Some(true));
    }
}
