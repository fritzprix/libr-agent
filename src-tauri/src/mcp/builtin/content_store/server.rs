// server.rs - ContentStoreServer implementation
use crate::mcp::types::{ServiceContext, ServiceContextOptions};
use crate::mcp::MCPTool;
use crate::session::SessionManager;
use log::error;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{schemas, search, storage};

/// Content-Store built-in MCP server (native backend)
#[derive(Debug)]
pub struct ContentStoreServer {
    #[allow(dead_code)]
    pub(crate) session_id: String,
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) storage: Mutex<storage::ContentStoreStorage>,
    pub(crate) search_engine: Arc<Mutex<search::ContentSearchEngine>>,
}

impl ContentStoreServer {
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        let session_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
        let search_index_dir = session_dir.join("content_store_search");
        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        Self {
            session_id,
            session_manager,
            storage: Mutex::new(storage::ContentStoreStorage::new()),
            search_engine: Arc::new(Mutex::new(search_engine)),
        }
    }

    pub async fn new_with_sqlite(
        session_id: String,
        session_manager: Arc<SessionManager>,
        database_url: String,
    ) -> Result<Self, String> {
        let session_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
        let search_index_dir = session_dir.join("content_store_search");
        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let storage = storage::ContentStoreStorage::new_sqlite(database_url).await?;

        Ok(Self {
            session_id,
            session_manager,
            storage: Mutex::new(storage),
            search_engine: Arc::new(Mutex::new(search_engine)),
        })
    }

    pub async fn new_with_db(
        session_id: String,
        session_manager: Arc<SessionManager>,
        db: sea_orm::DatabaseConnection,
    ) -> Result<Self, String> {
        let session_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
        let search_index_dir = session_dir.join("content_store_search");
        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let storage = storage::ContentStoreStorage::new_with_db(db).await?;

        Ok(Self {
            session_id,
            session_manager,
            storage: Mutex::new(storage),
            search_engine: Arc::new(Mutex::new(search_engine)),
        })
    }

    pub fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "saveKnowledge".to_string(),
                title: Some("Save Knowledge".to_string()),
                description: "Save knowledge entry (text or file) to the content store".to_string(),
                input_schema: serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "Title of the knowledge entry" },
                        "content": { "type": "string", "description": "Content to save" },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Tags for the entry" },
                        "fileUrl": { "type": "string", "description": "File URL (file://) to add" },
                        "srcUrl": { "type": "string", "description": "Source URL" },
                        "metadata": { "type": "object", "description": "Additional metadata" }
                    },
                    "required": ["content"]
                })).unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "listContent".to_string(),
                title: Option::None,
                description: "List content in a store with pagination".to_string(),
                input_schema: schemas::tool_list_content_schema(),
                output_schema: Option::None,
                annotations: Option::None,
            },
            MCPTool {
                name: "readContent".to_string(),
                title: Option::None,
                description: "Read content with line range filtering".to_string(),
                input_schema: schemas::tool_read_content_schema(),
                output_schema: Option::None,
                annotations: Option::None,
            },
            MCPTool {
                name: "searchKnowledge".to_string(),
                title: Some("Search Knowledge".to_string()),
                description: "Search for knowledge entries using keywords".to_string(),
                input_schema: serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "options": { 
                            "type": "object",
                            "description": "Search options",
                            "properties": {
                                "topN": { "type": "integer" },
                                "threshold": { "type": "number" }
                            }
                        }
                    },
                    "required": ["query"]
                })).unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "deleteContent".to_string(),
                title: Option::None,
                description: "Remove content from a store".to_string(),
                input_schema: schemas::tool_delete_content_schema(),
                output_schema: Option::None,
                annotations: Option::None,
            },
        ]
    }

    /// Get the session ID for this server instance
    ///
    /// In the new multi-session architecture, each ContentStoreServer is bound to a specific
    /// session at construction time. This method returns that session ID.
    ///
    /// For legacy compatibility, if session_manager has a current session set via switch_context,
    /// that takes precedence. Otherwise, returns the constructor-bound session_id.
    pub(crate) fn require_active_session_result(&self) -> Result<String, String> {
        // For legacy compatibility: check if session_manager has an active session
        if let Some(session_id) = self.session_manager.get_current_session() {
            Ok(session_id)
        } else {
            // New architecture: use the session_id bound at construction
            Ok(self.session_id.clone())
        }
    }

    pub(crate) async fn ensure_session_store(&self, session_id: &str) -> Result<(), String> {
        let mut storage = self.storage.lock().await;

        if storage.store_exists(session_id) {
            return Ok(());
        }

        storage
            .get_or_create_store(
                session_id.to_string(),
                Some(format!("Session Store: {session_id}")),
                Some(format!("Content store for session {session_id}")),
            )
            .await
            .map(|_| ())
    }

    pub async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Use session_id from constructor (already bound to this session)
        // This is consistent with Planning/Workspace pattern
        let session_id = &self.session_id;

        // Get content information for this session
        let count = match self.storage.try_lock() {
            Ok(storage) => storage.get_content_count(session_id),
            Err(e) => {
                log::warn!(
                    "Failed to lock content storage for session '{}': {}",
                    session_id,
                    e
                );
                return ServiceContext {
                    context_prompt: "## Content Store\n\nError loading state".to_string(),
                    structured_state: None,
                };
            }
        };

        // Build context prompt (Legacy style: concise, token-efficient)
        let file_status = if count == 0 {
            "no files".to_string()
        } else if count == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", count)
        };

        // Tool count (fixed: saveKnowledge, listContent, readContent, searchKnowledge, deleteContent)
        let tool_count = 5;

        let context_prompt = format!(
            "## Content Store\n\nActive, {} tools, {}",
            tool_count, file_status
        );

        ServiceContext {
            context_prompt,
            structured_state: Some(serde_json::json!({
                "active": true,
                "tool_count": tool_count,
                "file_count": count,
                "session_id": session_id
            })),
        }
    }

    pub async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> {
        if let Some(session_id) = &options.session_id {
            // Use the async session setter to avoid blocking and to allow the caller
            // to cancel the operation by dropping the awaiting future.
            if let Err(e) = self
                .session_manager
                .set_session_async(session_id.clone())
                .await
            {
                error!("Failed to switch session in session_manager: {e}");
                return Err(format!("Failed to switch session in session_manager: {e}"));
            }

            let mut storage = self.storage.lock().await;

            match storage
                .get_or_create_store(
                    session_id.clone(),
                    Some(format!("Session Store: {session_id}")),
                    Some(format!("Content store for session {session_id}")),
                )
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to get or create content store for session {session_id}: {e}");
                    return Err(format!(
                        "Failed to get or create content store for session {session_id}: {e}"
                    ));
                }
            }
        }

        Ok(())
    }
}
