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
mod tools;

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

        let repo = crate::get_assistant_repository();
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
    use sea_orm::{ConnectionTrait, Database, Schema};
    use serde_json::json;

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
