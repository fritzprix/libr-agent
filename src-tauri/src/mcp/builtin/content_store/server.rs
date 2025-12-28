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
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) storage: Mutex<storage::ContentStoreStorage>,
    pub(crate) search_engine: Arc<Mutex<search::ContentSearchEngine>>,
}

impl ContentStoreServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        let session_dir = session_manager.get_session_workspace_dir();
        let search_index_dir = session_dir.join("content_store_search");
        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        Self {
            session_manager,
            storage: Mutex::new(storage::ContentStoreStorage::new()),
            search_engine: Arc::new(Mutex::new(search_engine)),
        }
    }

    pub async fn new_with_sqlite(
        session_manager: Arc<SessionManager>,
        database_url: String,
    ) -> Result<Self, String> {
        let session_dir = session_manager.get_session_workspace_dir();
        let search_index_dir = session_dir.join("content_store_search");
        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let storage = storage::ContentStoreStorage::new_sqlite(database_url).await?;

        Ok(Self {
            session_manager,
            storage: Mutex::new(storage),
            search_engine: Arc::new(Mutex::new(search_engine)),
        })
    }

    pub fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "addContent".to_string(),
                title: Option::None,
                description: "Add and parse file content with chunking and BM25 indexing"
                    .to_string(),
                input_schema: schemas::tool_add_content_schema(),
                output_schema: Option::None,
                annotations: Option::None,
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
                name: "keywordSimilaritySearch".to_string(),
                title: Option::None,
                description: "Perform BM25-based keyword search across stored content".to_string(),
                input_schema: schemas::tool_keyword_search_schema(),
                output_schema: Option::None,
                annotations: Option::None,
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

    /// New Result-based helper for require_active_session
    pub(crate) fn require_active_session_result(&self) -> Result<String, String> {
        if let Some(session_id) = self.session_manager.get_current_session() {
            Ok(session_id)
        } else {
            Err("No active session context. Call switch_context with a sessionId before invoking this tool.".to_string())
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
        // Get current session ID
        let session_id = match self.session_manager.get_current_session() {
            Some(sid) => sid,
            None => {
                return ServiceContext {
                    context_prompt: "## Content Store\n**Status**: No active session".to_string(),
                    structured_state: None,
                };
            }
        };

        // Get content information for this session
        let (count, summaries) = match self.storage.try_lock() {
            Ok(storage) => {
                let count = storage.get_content_count(&session_id);
                let summaries = storage.get_content_summary(&session_id, 5);
                (count, summaries)
            }
            Err(e) => {
                log::warn!(
                    "Failed to lock content storage for session '{}': {}",
                    session_id,
                    e
                );
                return ServiceContext {
                    context_prompt: "## Content Store\n**Status**: Error loading state".to_string(),
                    structured_state: None,
                };
            }
        };

        // Build context prompt
        let mut parts = vec!["## Content Store".to_string()];

        if count == 0 {
            parts.push("\n**No content stored yet.**".to_string());
            parts.push(
                "*Use addContent to store files, documents, or text for later retrieval.*"
                    .to_string(),
            );
        } else {
            let file_label = if count == 1 { "file" } else { "files" };
            parts.push(format!("\n**{} {} stored**", count, file_label));

            // List content items with previews
            for (idx, (filename, size, preview)) in summaries.iter().enumerate() {
                // Format size in human-readable form
                let size_str = if *size < 1024 {
                    format!("{}B", size)
                } else if *size < 1024 * 1024 {
                    format!("{}KB", size / 1024)
                } else {
                    format!("{}MB", size / (1024 * 1024))
                };

                // Truncate preview to 50 chars
                let preview_short = if preview.len() > 50 {
                    format!("{}...", &preview[..50])
                } else {
                    preview.clone()
                };

                parts.push(format!(
                    "  {}. **{}** ({}) - {}",
                    idx + 1,
                    filename,
                    size_str,
                    preview_short
                ));
            }

            if count > 5 {
                parts.push(format!(
                    "  ...and {} more files. Use listContent to view all.",
                    count - 5
                ));
            }
        }

        ServiceContext {
            context_prompt: parts.join("\n"),
            structured_state: Some(serde_json::json!({
                "session_id": session_id,
                "content_count": count
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
