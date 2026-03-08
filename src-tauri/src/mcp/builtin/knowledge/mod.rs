use async_trait::async_trait;
use sea_orm::*;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, MCPTool, ServiceContext};
use crate::repositories::KnowledgeRepository;

mod helpers;
mod operations;
mod queries;
mod tools;

/// Knowledge Server - Session-scoped knowledge base with full-text search
///
/// This server provides session-specific knowledge storage and retrieval using
/// SQLite FTS5 for efficient full-text search.
#[derive(Debug)]
pub struct KnowledgeServer {
    assistant_id: String,
    db: Arc<DatabaseConnection>,
}

impl KnowledgeServer {
    /// Create a new KnowledgeServer instance for a specific assistant
    pub async fn new(assistant_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        let server = Self { assistant_id, db };

        Ok(server)
    }

    pub(crate) fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Get tools statically (without an instance)
    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    /// Get metadata statically (without an instance)
    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Knowledge Server".to_string(),
            description: "Persistent knowledge base scoped to this assistant (survives across sessions). Use for information that should be remembered long-term. For session-only files, use the attachments server.".to_string(),
            icon: Some("📚".to_string()),
        }
    }
}

pub const NAME: &str = "knowledge";

#[async_trait]
impl BuiltinMCPServer for KnowledgeServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Persistent knowledge base scoped to this assistant (survives across sessions). Use for information that should be remembered long-term. For session-only files, use the attachments server."
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn display_name(&self) -> String {
        "Knowledge Server".to_string()
    }

    fn metadata(&self) -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: self.display_name(),
            description: self.description().to_string(),
            icon: Some("📚".to_string()),
        }
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Query knowledge count with error handling using repository
        let repo = crate::get_knowledge_repository();
        let assistant_count: u64 = repo.count_knowledge(&self.assistant_id).await.unwrap_or(0);
        let global_count: u64 = repo.count_knowledge("global").await.unwrap_or(0);

        // Build context prompt
        let mut parts = vec!["## Knowledge Base".to_string()];
        parts.push("This assistant has access to a persistent knowledge base. Knowledge can be stored in two scopes:".to_string());
        parts.push(
            "- **global**: Shared across all assistants and sessions. (Default for saving)"
                .to_string(),
        );
        parts.push(
            format!(
                "- **assistant**: Private to this specific assistant ({}).",
                self.assistant_id
            )
            .to_string(),
        );

        if assistant_count == 0 && global_count == 0 {
            parts.push("\n**No knowledge entries yet.**".to_string());
            parts.push(
                "*Use saveKnowledge to store important information for future reference (defaults to global scope).*".to_string(),
            );
        } else {
            parts.push(format!(
                "\n**Available knowledge: {} global, {} assistant-specific entries.**",
                global_count, assistant_count
            ));
            parts.push(
                "Use searchKnowledge or listKnowledge to explore available information."
                    .to_string(),
            );
        }

        ServiceContext {
            context_prompt: parts.join("\n"),
            structured_state: Some(json!({
                "assistant_id": self.assistant_id,
                "assistant_knowledge_count": assistant_count,
                "global_knowledge_count": global_count
            })),
        }
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        log::debug!(
            "Knowledge server tool called: {} for session: {}",
            tool_name,
            _session_id.as_deref().unwrap_or("none")
        );

        // Determine target ID based on scope parameter
        let scope = args
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        // For search/list, "default" means "both"
        // For save/read/delete, "default" means "global" (to fulfill sharing requirement)
        let target_assistant_id = match scope {
            "global" => "global".to_string(),
            "assistant" => self.assistant_id.clone(),
            _ => {
                if tool_name == "saveKnowledge"
                    || tool_name == "readKnowledge"
                    || tool_name == "deleteKnowledge"
                {
                    "global".to_string()
                } else {
                    // For search/list, we'll handle "both" logic inside match
                    "global".to_string()
                }
            }
        };

        match tool_name {
            "saveKnowledge" => {
                operations::save_knowledge(self, args, &target_assistant_id).await
            }
            "readKnowledge" => {
                queries::read_knowledge(self, args, &target_assistant_id).await
            }
            "deleteKnowledge" => {
                operations::delete_knowledge(self, args, &target_assistant_id).await
            }
            "searchKnowledge" => {
                if scope == "both" || scope == "default" {
                    // When searching with scope "both" or "default", perform a combined search
                    // across global and assistant-specific knowledge using the current assistant ID.
                    queries::search_knowledge_both(self, args, &self.assistant_id).await
                } else {
                    queries::search_knowledge(self, args, &target_assistant_id).await
                }
            }
            "listKnowledge" => {
                if scope == "both" || scope == "default" {
                    queries::list_knowledge_both(self, args, &self.assistant_id).await
                } else {
                    queries::list_knowledge(self, args, &target_assistant_id).await
                }
            }
            _ => Err(format!(
                "Unknown tool: {}. Available tools: saveKnowledge, readKnowledge, deleteKnowledge, searchKnowledge, listKnowledge",
                tool_name
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{knowledge, session};
    use sea_orm::{
        ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, Schema, Set,
        Statement,
    };
    use std::sync::Arc;

    async fn create_test_db() -> Arc<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory database");

        let schema = Schema::new(db.get_database_backend());

        // Create sessions table
        let stmt = schema.create_table_from_entity(session::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create sessions table");

        // Create knowledge table
        let stmt = schema.create_table_from_entity(knowledge::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create knowledge table");

        // Create FTS table manually for testing search
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE VIRTUAL TABLE knowledge_fts USING fts5(title, content, tags, source, content='knowledge', content_rowid='id');".to_owned(),
        ))
        .await
        .expect("Failed to create FTS table");

        // Triggers for FTS sync
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TRIGGER knowledge_ai AFTER INSERT ON knowledge BEGIN
              INSERT INTO knowledge_fts(rowid, title, content, tags, source) VALUES (new.id, new.title, new.content, new.tags, new.source);
            END;".to_owned(),
        )).await.expect("Failed to create insert trigger");

        // Insert test sessions
        let sessions = vec!["test-session", "session-1", "session-2"];
        for id in sessions {
            let new_session = session::ActiveModel {
                id: Set(id.to_string()),
                status: Set("active".to_string()),
                created_at: Set(0),
                updated_at: Set(0),
                ..Default::default()
            };
            session::Entity::insert(new_session)
                .exec(&db)
                .await
                .expect("Failed to insert session");
        }

        Arc::new(db)
    }

    #[tokio::test]
    async fn test_save_and_search_knowledge() {
        let db = create_test_db().await;
        let server = KnowledgeServer::new("test-session".to_string(), db)
            .await
            .expect("Failed to create server");

        // Save knowledge
        let result = server
            .call_tool(
                "saveKnowledge",
                json!({
                    "title": "Rust Ownership",
                    "content": "Rust uses an ownership system to manage memory safety",
                    "tags": ["rust", "programming"]
                }),
                None,
            )
            .await
            .expect("Save should succeed");

        assert_eq!(result.is_error, Some(false));

        // Search for it
        let result = server
            .call_tool("searchKnowledge", json!({"query": "ownership"}), None)
            .await
            .expect("Search should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["count"], 1);
    }

    #[tokio::test]
    async fn test_list_knowledge() {
        let db = create_test_db().await;
        let server = KnowledgeServer::new("test-session".to_string(), db)
            .await
            .expect("Failed to create server");

        // Save multiple entries
        for i in 1..=3 {
            server
                .call_tool(
                    "saveKnowledge",
                    json!({
                        "title": format!("Entry {}", i),
                        "content": format!("Content {}", i)
                    }),
                    None,
                )
                .await
                .expect("Save should succeed");
        }

        // List all
        let result = server
            .call_tool("listKnowledge", json!({}), None)
            .await
            .expect("List should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["count"], 3);
    }

    #[tokio::test]
    async fn test_session_isolation() {
        let db = create_test_db().await;

        let server1 = KnowledgeServer::new("session-1".to_string(), db.clone())
            .await
            .expect("Failed to create server1");

        let server2 = KnowledgeServer::new("session-2".to_string(), db)
            .await
            .expect("Failed to create server2");

        // Save to session 1
        server1
            .call_tool(
                "saveKnowledge",
                json!({"title": "Session 1 Data", "content": "Private"}),
                None,
            )
            .await
            .expect("Save should succeed");

        // List in session 2 - should be empty
        let result = server2
            .call_tool("listKnowledge", json!({}), None)
            .await
            .expect("List should succeed");

        let structured = result.structured_content.unwrap();
        assert_eq!(structured["count"], 0);
    }

    #[tokio::test]
    async fn test_knowledge_source_and_snippet() {
        let db = create_test_db().await;
        let server = KnowledgeServer::new("test-session".to_string(), db)
            .await
            .expect("Failed to create server");

        // Save knowledge with source
        let result = server
            .call_tool(
                "saveKnowledge",
                json!({
                    "title": "Document A",
                    "content": "This is a very important document content that should be snippeted. It contains specific keywords like pineapple and banana.",
                    "source": "http://example.com/doc-a",
                    "tags": ["test"]
                }),
                None,
            )
            .await
            .expect("Save should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = result.structured_content.unwrap();
        let knowledge = &structured["knowledge"];
        assert_eq!(knowledge["source"], "http://example.com/doc-a");

        // Search for it by keyword
        let result = server
            .call_tool("searchKnowledge", json!({"query": "pineapple"}), None)
            .await
            .expect("Search should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = result.structured_content.as_ref().unwrap();
        assert_eq!(structured["count"], 1);
        let first_result = &structured["results"][0];

        // Check source is returned
        assert_eq!(first_result["source"], "http://example.com/doc-a");

        // Check snippet is returned and contains the keyword
        let snippet = first_result["snippet"].as_str().unwrap();
        assert!(!snippet.is_empty());
        assert!(snippet.contains("pineapple") || snippet.contains("banana"));

        // Test filtering by source
        let result = server
            .call_tool(
                "searchKnowledge",
                json!({"source": "http://example.com/doc-a"}),
                None,
            )
            .await
            .expect("Source search should succeed");

        let structured = result.structured_content.as_ref().unwrap();
        assert_eq!(structured["count"], 1);

        // Test filtering by WRONG source
        let result = server
            .call_tool(
                "searchKnowledge",
                json!({"source": "http://example.com/wrong-doc"}),
                None,
            )
            .await
            .expect("Source search should succeed");

        let structured = result.structured_content.as_ref().unwrap();
        assert_eq!(structured["count"], 0);

        // Verify text response formatting
        if let Some(content_vec) = result.content {
            if let Some(crate::mcp::types::MCPContent::Text { text, .. }) = content_vec.first() {
                println!("Text response: {}", text);
                // Note: emptiness check depends on search result, but here count is 0 so it should be "Found 0..."
                assert!(text.contains("Found 0 knowledge entries"));
            }
        }
    }
}
