// server.rs - ContentStoreServer implementation
use crate::mcp::types::ServiceContext;
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
    #[allow(dead_code)]
    pub uploaded_at: String,
}

/// Content-Store built-in MCP server (native backend)
#[derive(Debug)]
pub struct ContentStoreServer {
    pub(crate) session_id: String,
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) storage: Mutex<storage::ContentStoreStorage>,
    pub(crate) search_engines: Mutex<HashMap<String, Arc<Mutex<search::ContentSearchEngine>>>>,
    /// Track recent uploads for service context (FIFO, max 10 items)
    pub(crate) recent_uploads: Arc<Mutex<VecDeque<RecentUploadInfo>>>,
}

impl ContentStoreServer {
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        let session_dir = session_manager.get_session_workspace_dir_by_id(&session_id);
        let search_index_dir = session_dir.join("content_store_search");
        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let mut search_engines = HashMap::new();
        search_engines.insert(session_id.clone(), Arc::new(Mutex::new(search_engine)));

        Self {
            session_id,
            session_manager,
            storage: Mutex::new(storage::ContentStoreStorage::new()),
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
        let search_index_dir = session_dir.join("content_store_search");
        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let storage = storage::ContentStoreStorage::new_sqlite(database_url).await?;

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
        let search_index_dir = session_dir.join("content_store_search");
        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let storage = storage::ContentStoreStorage::new_with_db(db).await?;

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
                name: "addContent".to_string(),
                title: Some("Add Content".to_string()),
                description: "Add content entry (text or file) to the content store".to_string(),
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
                name: "searchContent".to_string(),
                title: Some("Search Content".to_string()),
                description: "Search session-scoped content using BM25 keyword ranking. Only finds content uploaded in the current session.".to_string(),
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
            display_name: "Content Store".to_string(),
            description: "File attachment and semantic search system with native performance and BM25 indexing".to_string(),
            icon: Some("database".to_string()),
        }
    }

    pub fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
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

    pub(crate) async fn get_search_engine(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<search::ContentSearchEngine>>, String> {
        let mut engines = self.search_engines.lock().await;

        if let Some(engine) = engines.get(session_id) {
            return Ok(engine.clone());
        }

        // Create new search engine instance for this session
        let session_dir = self
            .session_manager
            .get_session_workspace_dir_by_id(session_id);
        let search_index_dir = session_dir.join("content_store_search");

        let search_engine = search::ContentSearchEngine::new(search_index_dir).map_err(|e| {
            format!(
                "Failed to initialize search engine for session {}: {}",
                session_id, e
            )
        })?;

        let engine_arc = Arc::new(Mutex::new(search_engine));
        engines.insert(session_id.to_string(), engine_arc.clone());

        Ok(engine_arc)
    }

    pub async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Use session_id from constructor (already bound to this session)
        let session_id = &self.session_id;

        // Get total content count
        let total_count = match self.storage.try_lock() {
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

        // Get recent uploads
        let recent_files = match self.recent_uploads.try_lock() {
            Ok(recent) => recent.iter().cloned().collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        };

        // Build context prompt with file details
        let mut prompt_parts = vec![
            "## Content Store\n".to_string(),
            format!(
                "{} available, 5 tools\n",
                Self::format_file_count(total_count)
            ),
        ];

        if !recent_files.is_empty() {
            prompt_parts.push("\n**Recent Uploads:**\n".to_string());

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

            prompt_parts.push("\n*Use `readContent(contentId=\"content_xxx\", fromLine=1, toLine=100)` to access files.*\n".to_string());
        } else if total_count == 0 {
            prompt_parts.push("*No files uploaded yet.*\n".to_string());
        }

        let context_prompt = prompt_parts.join("");

        ServiceContext {
            context_prompt,
            structured_state: Some(serde_json::json!({
                "active": true,
                "tool_count": 5,
                "file_count": total_count,
                "recent_uploads": recent_files.iter().map(|f| serde_json::json!({
                    "contentId": f.content_id,
                    "filename": f.filename,
                    "lineCount": f.line_count,
                })).collect::<Vec<_>>(),
            })),
        }
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
