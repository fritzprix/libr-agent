// server.rs - AttachmentsServer implementation
use crate::mcp::types::{ContextVolatility, ServiceContext};
use crate::mcp::MCPTool;
use crate::session::SessionManager;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{schemas, search, storage};

/// Information about a recently uploaded file for service context
#[derive(Debug, Clone)]
pub struct RecentUploadInfo {
    pub content_id: String,
    pub filename: String,
    pub mime_type: String,
    pub line_count: usize,
}

/// Attachments built-in MCP server (native backend)
#[derive(Debug)]
pub struct AttachmentsServer {
    pub(crate) session_id: String,
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) storage: Mutex<storage::AttachmentsStorage>,
    pub(crate) search_engines: Mutex<HashMap<String, Arc<Mutex<search::AttachmentSearchEngine>>>>,
    /// Track recent uploads for service context (FIFO, max 10 items)
    pub(crate) recent_uploads: Arc<Mutex<VecDeque<RecentUploadInfo>>>,
}

impl AttachmentsServer {
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        let session_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
        let search_index_dir = session_dir.join("attachments_search");
        let search_engine = search::AttachmentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let mut search_engines = HashMap::new();
        search_engines.insert(session_id.clone(), Arc::new(Mutex::new(search_engine)));

        Self {
            session_id,
            session_manager,
            storage: Mutex::new(storage::AttachmentsStorage::new()),
            search_engines: Mutex::new(search_engines),
            recent_uploads: Arc::new(Mutex::new(VecDeque::with_capacity(10))),
        }
    }

    pub async fn new_with_sqlite(
        session_id: String,
        session_manager: Arc<SessionManager>,
        database_url: String,
    ) -> Result<Self, String> {
        let session_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
        let search_index_dir = session_dir.join("attachments_search");
        let search_engine = search::AttachmentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let storage = storage::AttachmentsStorage::new_sqlite(database_url).await?;

        let mut search_engines = HashMap::new();
        search_engines.insert(session_id.clone(), Arc::new(Mutex::new(search_engine)));

        Ok(Self {
            session_id,
            session_manager,
            storage: Mutex::new(storage),
            search_engines: Mutex::new(search_engines),
            recent_uploads: Arc::new(Mutex::new(VecDeque::with_capacity(10))),
        })
    }

    pub async fn new_with_db(
        session_id: String,
        session_manager: Arc<SessionManager>,
        db: sea_orm::DatabaseConnection,
    ) -> Result<Self, String> {
        let session_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
        let search_index_dir = session_dir.join("attachments_search");
        let search_engine = search::AttachmentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let storage = storage::AttachmentsStorage::new_with_db(db).await?;

        let mut search_engines = HashMap::new();
        search_engines.insert(session_id.clone(), Arc::new(Mutex::new(search_engine)));

        Ok(Self {
            session_id,
            session_manager,
            storage: Mutex::new(storage),
            search_engines: Mutex::new(search_engines),
            recent_uploads: Arc::new(Mutex::new(VecDeque::with_capacity(10))),
        })
    }

    pub fn tools_static() -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "list".to_string(),
                title: Option::None,
                description: "List files attached to the current session with pagination".to_string(),
                input_schema: schemas::tool_list_content_schema(),
                output_schema: Option::None,
                annotations: Option::None,
            },
            MCPTool {
                name: "read".to_string(),
                title: Option::None,
                description: "Read attachment content with line range filtering".to_string(),
                input_schema: schemas::tool_read_content_schema(),
                output_schema: Option::None,
                annotations: Option::None,
            },
            MCPTool {
                name: "search".to_string(),
                title: Some("Search Attachments".to_string()),
                description: "Search session attachments using BM25 keyword ranking. Only finds files uploaded in the current session.".to_string(),
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
        ]
    }

    pub fn metadata_static() -> crate::mcp::types::BuiltinServerMetadata {
        crate::mcp::types::BuiltinServerMetadata {
            display_name: "Attachments".to_string(),
            description: "Session-scoped file attachment and search system".to_string(),
            icon: Some("paperclip".to_string()),
        }
    }

    pub fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    pub async fn add_attachment_internal(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<crate::mcp::types::MCPResult, String> {
        super::operations::add_content(self, args, session_id).await
    }

    pub async fn delete_attachment_internal(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<crate::mcp::types::MCPResult, String> {
        super::operations::delete_content(self, args, session_id).await
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
                Some(format!("Attachment store for session {session_id}")),
            )
            .await
            .map(|_| ())
    }

    pub(crate) async fn get_search_engine(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<search::AttachmentSearchEngine>>, String> {
        let mut engines = self.search_engines.lock().await;

        if let Some(engine) = engines.get(session_id) {
            return Ok(engine.clone());
        }

        // Create new search engine instance for this session
        let session_dir = self
            .session_manager
            .get_session_workspace_dir_by_id(session_id);
        let search_index_dir = session_dir.join("attachments_search");

        let search_engine = search::AttachmentSearchEngine::new(search_index_dir).map_err(|e| {
            format!(
                "Failed to initialize search engine for session {}: {}",
                session_id, e
            )
        })?;

        let engine_arc = Arc::new(Mutex::new(search_engine));
        engines.insert(session_id.to_string(), engine_arc.clone());

        Ok(engine_arc)
    }

    pub async fn get_service_context_internal(&self, _options: Option<&Value>) -> ServiceContext {
        // Use session_id from constructor (already bound to this session)
        let session_id = &self.session_id;

        // Get total content count
        let total_count = match self.storage.try_lock() {
            Ok(storage) => storage.get_content_count(session_id),
            Err(e) => {
                log::warn!(
                    "Failed to lock attachment storage for session '{}': {}",
                    session_id,
                    e
                );
                return ServiceContext::new(
                    "## Attachments\n\n### Live State\n- Error loading state",
                )
                .with_volatility(ContextVolatility::Volatile);
            }
        };

        // Get recent uploads
        let recent_files = match self.recent_uploads.try_lock() {
            Ok(recent) => recent.iter().cloned().collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        };

        // Build context prompt with file details
        let mut prompt_parts = vec![
            "## Attachments\n".to_string(),
            "### Live State\n".to_string(),
            format!(
                "- {} available, 5 tools\n",
                Self::format_file_count(total_count)
            ),
        ];

        if !recent_files.is_empty() {
            prompt_parts.push("\n**Recently Attached:**\n".to_string());

            for (i, file) in recent_files.iter().take(10).enumerate() {
                prompt_parts.push(format!(
                    "{}. `{}` (ID: `{}`, {} lines, {})\n",
                    i + 1,
                    file.filename,
                    file.content_id,
                    file.line_count,
                    Self::format_mime_type(&file.mime_type)
                ));
            }
        } else if total_count == 0 {
            prompt_parts.push("Attachments: None\n".to_string());
        }

        let context_prompt = prompt_parts.join("");

        ServiceContext::new(context_prompt)
            .with_structured_state(serde_json::json!({
                "active": true,
                "tool_count": 5,
                "file_count": total_count,
                "recent_uploads": recent_files.iter().map(|f| serde_json::json!({
                    "contentId": f.content_id,
                    "filename": f.filename,
                    "lineCount": f.line_count,
                })).collect::<Vec<_>>(),
            }))
            .with_volatility(ContextVolatility::Volatile)
    }

    // Helper functions for service context formatting
    pub(crate) fn format_file_count(count: usize) -> String {
        match count {
            0 => "No files".to_string(),
            1 => "1 file".to_string(),
            n => format!("{} files", n),
        }
    }

    pub(crate) fn normalize_content_id(id: &str) -> String {
        // "add24ru333bbupvroeea53qj" → "content_add24ru333bbupvroeea53qj"
        if id.starts_with("content_") {
            id.to_string()
        } else {
            format!("content_{}", id)
        }
    }

    pub(crate) fn format_mime_type(mime: &str) -> String {
        match mime {
            "text/plain" => "text".to_string(),
            "text/markdown" => "markdown".to_string(),
            "application/json" => "JSON".to_string(),
            "application/pdf" => "PDF".to_string(),
            _ => mime.to_string(),
        }
    }
}
