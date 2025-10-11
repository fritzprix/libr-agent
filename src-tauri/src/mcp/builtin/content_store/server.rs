// server.rs - ContentStoreServer implementation
use crate::mcp::types::{ServiceContext, ServiceContextOptions};
use crate::mcp::{MCPResponse, MCPTool};
use crate::session::SessionManager;
use log::{error, info};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{schemas, search, storage, utils};

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
                title: None,
                description: "Add and parse file content with chunking and BM25 indexing"
                    .to_string(),
                input_schema: schemas::tool_add_content_schema(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "listContent".to_string(),
                title: None,
                description: "List content in a store with pagination".to_string(),
                input_schema: schemas::tool_list_content_schema(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "readContent".to_string(),
                title: None,
                description: "Read content with line range filtering".to_string(),
                input_schema: schemas::tool_read_content_schema(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "keywordSimilaritySearch".to_string(),
                title: None,
                description: "Perform BM25-based keyword search across stored content".to_string(),
                input_schema: schemas::tool_keyword_search_schema(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "deleteContent".to_string(),
                title: None,
                description: "Remove content from a store".to_string(),
                input_schema: schemas::tool_delete_content_schema(),
                output_schema: None,
                annotations: None,
            },
        ]
    }

    // Utility methods
    pub(crate) fn generate_request_id() -> Value {
        utils::generate_request_id()
    }

    pub(crate) fn dual_response(
        request_id: Value,
        message: &str,
        structured_content: Value,
    ) -> MCPResponse {
        utils::create_dual_response(request_id, message, structured_content)
    }

    pub(crate) fn error_response(request_id: Value, code: i32, message: &str) -> MCPResponse {
        utils::create_error_response(request_id, code, message)
    }

    pub(crate) fn require_active_session(
        &self,
        request_id: &Value,
    ) -> Result<String, Box<MCPResponse>> {
        if let Some(session_id) = self.session_manager.get_current_session() {
            Ok(session_id)
        } else {
            Err(Box::new(Self::error_response(
                request_id.clone(),
                -32002,
                "No active session context. Call switch_context with a sessionId before invoking this tool.",
            )))
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

    pub fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        info!("ContentStore get_service_context called with options: {options:?}");

        // Extract session ID from options if provided
        let session_id = options
            .and_then(|opts| opts.get("sessionId"))
            .and_then(|sid| sid.as_str())
            .filter(|s| !s.is_empty());

        // Get basic server information
        let tools_count = self.tools().len();

        // Format minimal context with content summary
        let mut context = format!("contentstore: Active, {tools_count} tools");

        // Add session-specific content summary if session ID is available
        if let Some(session_id) = session_id {
            // Try to get content information for this session (non-blocking)
            if let Ok(storage) = self.storage.try_lock() {
                let count = storage.get_content_count(session_id);
                let summaries = storage.get_content_summary(session_id, 3);

                if count > 0 {
                    context.push_str(&format!(", {count} files"));

                    if !summaries.is_empty() {
                        let files_info: Vec<String> = summaries
                            .iter()
                            .map(|(filename, size, preview)| {
                                // Format size in human-readable form
                                let size_str = if *size < 1024 {
                                    format!("{size}B")
                                } else if *size < 1024 * 1024 {
                                    format!("{}KB", size / 1024)
                                } else {
                                    format!("{}MB", size / (1024 * 1024))
                                };

                                format!("{filename} ({size_str}): {preview}")
                            })
                            .collect();

                        context.push_str(&format!("\n  Files: {}", files_info.join("\n  ")));

                        if summaries.len() < count {
                            context
                                .push_str(&format!("\n  ...and {} more", count - summaries.len()));
                        }
                    }
                } else {
                    context.push_str(", no files");
                }
            } else {
                context.push_str(", content info unavailable");
            }
        } else {
            context.push_str(", no session");
        }

        info!("ContentStore service context - detailed format with file summary");

        ServiceContext {
            context_prompt: context,
            structured_state: session_id.map(|s| Value::String(s.to_string())),
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
