use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::mcp::schema::JSONSchema;
use crate::mcp::utils::schema_builder::integer_prop;
use crate::mcp::{MCPResponse, MCPTool};
use crate::session::SessionManager;

use super::BuiltinMCPServer;

pub mod parsers;
pub mod search;
pub mod storage;
pub mod utils;

/// Helper function to extract file path from file:// URL
fn extract_file_path_from_url(file_url: &str) -> Result<String, String> {
    if let Some(path) = file_url.strip_prefix("file://") {
        // URL decode the path if needed
        Ok(path.to_string())
    } else {
        Err(format!("Invalid file URL format: {file_url}"))
    }
}

/// Tool argument types
#[derive(Debug, serde::Deserialize)]
struct CreateStoreArgs {
    metadata: Option<CreateStoreMetadata>,
}

#[derive(Debug, serde::Deserialize)]
struct CreateStoreMetadata {
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "sessionId", alias = "session_id")]
    session_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct AddContentArgs {
    #[serde(rename = "storeId", alias = "store_id")]
    store_id: String,
    #[serde(rename = "fileUrl", alias = "file_url")]
    file_url: Option<String>,
    content: Option<String>,
    metadata: Option<AddContentMetadata>,
}

#[derive(Debug, serde::Deserialize)]
struct AddContentMetadata {
    filename: Option<String>,
    #[serde(rename = "mimeType", alias = "mime_type")]
    mime_type: Option<String>,
    size: Option<u64>,
    #[serde(rename = "uploadedAt", alias = "uploaded_at")]
    uploaded_at: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct ListContentArgs {
    #[serde(rename = "storeId", alias = "store_id")]
    store_id: String,
}

#[derive(Debug, serde::Deserialize)]
struct ReadContentArgs {
    content_id: String,
    from_line: Option<usize>,
    to_line: Option<usize>,
}

#[derive(Debug, serde::Deserialize)]
struct KeywordSearchArgs {
    #[serde(rename = "storeId", alias = "store_id")]
    store_id: String,
    query: String,
}

/// Content-Store built-in MCP server (native backend)
#[derive(Debug)]
pub struct ContentStoreServer {
    #[allow(dead_code)]
    session_manager: Arc<SessionManager>,
    storage: Mutex<storage::ContentStoreStorage>,
    search_engine: Arc<Mutex<search::ContentSearchEngine>>,
}

impl ContentStoreServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        // Create search index directory in session workspace
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

    /// Create a new content store server with SQLite storage
    pub async fn new_with_sqlite(
        session_manager: Arc<SessionManager>,
        database_url: String,
    ) -> Result<Self, String> {
        // Create search index directory in session workspace
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

    // Common response creation methods (wrappers)
    pub fn generate_request_id() -> Value {
        utils::generate_request_id()
    }

    pub fn dual_response(
        request_id: Value,
        message: &str,
        structured_content: Value,
    ) -> MCPResponse {
        utils::create_dual_response(request_id, message, structured_content)
    }

    pub fn error_response(request_id: Value, code: i32, message: &str) -> MCPResponse {
        utils::create_error_response(request_id, code, message)
    }
}

#[async_trait]
impl BuiltinMCPServer for ContentStoreServer {
    fn name(&self) -> &str {
        "contentstore"
    }

    fn description(&self) -> &str {
        "File attachment and semantic search system with native performance and BM25 indexing"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "createStore".to_string(),
                title: None,
                description: "Create a new content store for file management".to_string(),
                input_schema: Self::tool_create_store_schema(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "addContent".to_string(),
                title: None,
                description: "Add and parse file content with chunking and BM25 indexing"
                    .to_string(),
                input_schema: Self::tool_add_content_schema(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "listContent".to_string(),
                title: None,
                description: "List content in a store with pagination".to_string(),
                input_schema: Self::tool_list_content_schema(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "readContent".to_string(),
                title: None,
                description: "Read content with line range filtering".to_string(),
                input_schema: Self::tool_read_content_schema(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "keywordSimilaritySearch".to_string(),
                title: None,
                description: "Perform BM25-based keyword search across stored content".to_string(),
                input_schema: Self::tool_keyword_search_schema(),
                output_schema: None,
                annotations: None,
            },
        ]
    }

    fn get_service_context(&self, options: Option<&Value>) -> String {
        // Extract session ID from options if provided
        let session_id = options
            .and_then(|opts| opts.get("sessionId"))
            .and_then(|sid| sid.as_str())
            .filter(|s| !s.is_empty());

        // Get basic server information
        let tools_count = self.tools().len();

        // Try to get storage statistics (non-blocking attempt)
        let storage_stats = if let Ok(_storage) = self.storage.try_lock() {
            // For now, we can't easily get total stores/contents without async
            // This is a limitation of the synchronous trait method
            "Available (detailed stats require async access)".to_string()
        } else {
            "Busy (storage locked)".to_string()
        };

        // Get search engine status
        let search_status = if let Ok(_search_engine) = self.search_engine.try_lock() {
            "Active and ready".to_string()
        } else {
            "Busy (search engine locked)".to_string()
        };

        // Format the context information following WorkspaceServer pattern
        let mut context = format!(
            "# Content Store Server Status\n\
            **Server**: contentstore\n\
            **Status**: Active\n\
            **Description**: File attachment and semantic search system\n\
            **Available Tools**: {tools_count} tools\n\
            **Storage**: {storage_stats}\n\
            **Search Engine**: {search_status}\n\
            "
        );

        // Add session-specific information if session ID is available
        if let Some(session_id) = session_id {
            context.push_str(&format!(
                "\n## Current Session\n\
                **Session ID**: {session_id}\n\
                **Store Mapping**: Using session ID as store ID (simplified)\n\
                "
            ));

            // Add note about attached files (but can't fetch them synchronously)
            context.push_str(
                "\n## Attached Files\n\
                *Note: File details require async access - use listContent tool for current session files*\n"
            );
        } else {
            context.push_str(
                "\n## Session Context\n\
                *No session ID provided in options - session-specific features unavailable*\n",
            );
        }

        // Add available tools information
        context.push_str(
            "\n## Available Tools\n\
            - **createStore**: Create a new content store\n\
            - **addContent**: Add and parse files with BM25 indexing\n\
            - **listContent**: List stored content with pagination\n\
            - **readContent**: Read content with line range filtering\n\
            - **keywordSimilaritySearch**: BM25-based keyword search\n",
        );

        context
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "createStore" => self.handle_create_store(args).await,
            "addContent" => self.handle_add_content(args).await,
            "listContent" => self.handle_list_content(args).await,
            "readContent" => self.handle_read_content(args).await,
            "keywordSimilaritySearch" => self.handle_keyword_search(args).await,
            _ => {
                let id = Self::generate_request_id();
                Self::error_response(id, -32601, &format!("Unknown tool: {tool_name}"))
            }
        }
    }
}

impl ContentStoreServer {
    fn tool_create_store_schema() -> JSONSchema {
        let mut meta_props: HashMap<String, JSONSchema> = HashMap::new();
        meta_props.insert("name".to_string(), string_prop(Some("Store display name")));
        meta_props.insert(
            "description".to_string(),
            string_prop(Some("Store description")),
        );
        meta_props.insert(
            "session_id".to_string(),
            string_prop(Some("Session ID for the store")),
        );
        object_schema(meta_props, vec![])
    }

    fn tool_add_content_schema() -> JSONSchema {
        let mut props: HashMap<String, JSONSchema> = HashMap::new();
        props.insert(
            "store_id".to_string(),
            string_prop(Some("Store ID to add content to")),
        );
        props.insert(
            "file_url".to_string(),
            string_prop(Some("File URL (file://) to add")),
        );
        props.insert(
            "content".to_string(),
            string_prop(Some("Direct content to add")),
        );
        props.insert(
            "metadata".to_string(),
            object_schema(
                {
                    let mut meta_props: HashMap<String, JSONSchema> = HashMap::new();
                    meta_props.insert(
                        "filename".to_string(),
                        string_prop(Some("Content filename")),
                    );
                    meta_props.insert("mime_type".to_string(), string_prop(Some("MIME type")));
                    meta_props.insert(
                        "size".to_string(),
                        integer_prop(Some(0), None, Some("Content size in bytes")),
                    );
                    meta_props.insert(
                        "uploaded_at".to_string(),
                        string_prop(Some("Upload timestamp")),
                    );
                    meta_props
                },
                vec![],
            ),
        );
        object_schema(props, vec!["store_id".to_string()])
    }

    fn tool_list_content_schema() -> JSONSchema {
        let mut props: HashMap<String, JSONSchema> = HashMap::new();
        props.insert(
            "store_id".to_string(),
            string_prop(Some("Store ID to list content from")),
        );
        props.insert(
            "pagination".to_string(),
            object_schema(
                {
                    let mut pagination_props: HashMap<String, JSONSchema> = HashMap::new();
                    pagination_props.insert(
                        "offset".to_string(),
                        integer_prop(Some(0), None, Some("Pagination offset")),
                    );
                    pagination_props.insert(
                        "limit".to_string(),
                        integer_prop(Some(1), Some(1000), Some("Pagination limit")),
                    );
                    pagination_props
                },
                vec![],
            ),
        );
        object_schema(props, vec!["store_id".to_string()])
    }

    fn tool_read_content_schema() -> JSONSchema {
        let mut props: HashMap<String, JSONSchema> = HashMap::new();
        props.insert(
            "content_id".to_string(),
            string_prop(Some("Content ID to read")),
        );
        props.insert(
            "from_line".to_string(),
            integer_prop(Some(1), None, Some("Starting line number (1-based)")),
        );
        props.insert(
            "to_line".to_string(),
            integer_prop(Some(1), None, Some("Ending line number (optional)")),
        );
        object_schema(props, vec!["content_id".to_string()])
    }

    fn tool_keyword_search_schema() -> JSONSchema {
        let mut props: HashMap<String, JSONSchema> = HashMap::new();
        props.insert(
            "store_id".to_string(),
            string_prop(Some("Store ID to search in")),
        );
        props.insert(
            "query".to_string(),
            string_prop(Some("Search query string")),
        );
        object_schema(props, vec!["store_id".to_string(), "query".to_string()])
    }

    async fn handle_create_store(&self, params: Value) -> MCPResponse {
        let id = Self::generate_request_id();
        let args: CreateStoreArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Self::error_response(
                    id,
                    -32602,
                    &format!("Invalid create_store parameters: {e}"),
                );
            }
        };

        let mut storage = self.storage.lock().await;
        let store = match storage
            .create_store(
                args.metadata.as_ref().and_then(|m| m.name.clone()),
                args.metadata.as_ref().and_then(|m| m.description.clone()),
                args.metadata.as_ref().and_then(|m| m.session_id.clone()),
            )
            .await
        {
            Ok(store) => store,
            Err(e) => {
                return Self::error_response(id, -32603, &format!("Failed to create store: {e}"));
            }
        };

        Self::dual_response(
            id,
            &format!(
                "Store created successfully!\nStore ID: {}\nCreated At: {}\nStatus: Ready for content",
                store.id, store.created_at
            ),
            serde_json::json!({
                "storeId": store.id,
                "id": store.id,
                "createdAt": store.created_at,
                "name": store.name,
                "description": store.description
            }),
        )
    }

    async fn handle_add_content(&self, params: Value) -> MCPResponse {
        let id = Self::generate_request_id();

        // Debug: Log the incoming parameters
        info!(
            "DEBUG: handle_add_content params: {}",
            serde_json::to_string(&params).unwrap_or_else(|_| "Failed to serialize".to_string())
        );
        info!("DEBUG: params type: {}", params);

        // Try to extract fields manually for debugging
        if let Some(obj) = params.as_object() {
            info!("DEBUG: params keys: {:?}", obj.keys().collect::<Vec<_>>());
            for (key, value) in obj {
                info!("DEBUG: {} = {:?}", key, value);
            }
        }

        let args: AddContentArgs = match serde_json::from_value(params) {
            Ok(args) => {
                info!("DEBUG: Parsed AddContentArgs: {:?}", args);
                args
            }
            Err(e) => {
                info!("DEBUG: Failed to parse AddContentArgs: {}", e);
                info!("DEBUG: Error details: {:?}", e);
                return Self::error_response(
                    id,
                    -32602,
                    &format!("Invalid add_content parameters: {e}"),
                );
            }
        };

        // Extract metadata information
        let metadata = args.metadata.as_ref();
        let filename = metadata.and_then(|m| m.filename.clone());
        let mime_type_from_metadata = metadata.and_then(|m| m.mime_type.clone());
        let size_from_metadata = metadata.and_then(|m| m.size);
        let uploaded_at = metadata.and_then(|m| m.uploaded_at.clone());

        info!("DEBUG: Extracted metadata - filename: {:?}, mime_type: {:?}, size: {:?}, uploaded_at: {:?}",
              filename, mime_type_from_metadata, size_from_metadata, uploaded_at);

        // Validate that either content or fileUrl is provided, but not both
        let content_text = match (&args.content, &args.file_url) {
            (Some(content), None) => {
                // Use provided content directly
                content.clone()
            }
            (None, Some(file_url)) => {
                // Extract file path from URL and parse the file
                let file_path_str = match extract_file_path_from_url(file_url) {
                    Ok(path) => path,
                    Err(e) => {
                        return Self::error_response(id, -32602, &format!("Invalid file URL: {e}"));
                    }
                };

                // Determine MIME type from file extension
                let mime_type = match std::path::Path::new(&file_path_str).extension() {
                    Some(ext) => match ext.to_str().unwrap_or("").to_lowercase().as_str() {
                        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                        "pdf" => "application/pdf",
                        "txt" => "text/plain",
                        "md" => "text/markdown",
                        "csv" => "text/csv",
                        _ => "text/plain", // Default fallback
                    },
                    None => "text/plain",
                };

                // Parse the file
                let file_path = std::path::Path::new(&file_path_str);
                match parsers::DocumentParser::parse_file(file_path, mime_type).await {
                    parsers::ParseResult::Text(content) => content,
                    parsers::ParseResult::Error(e) => {
                        return Self::error_response(
                            id,
                            -32603,
                            &format!("Failed to parse file {file_path_str}: {e}"),
                        );
                    }
                }
            }
            (Some(_), Some(_)) => {
                return Self::error_response(
                    id,
                    -32602,
                    "Cannot provide both content and fileUrl. Choose one.",
                );
            }
            (None, None) => {
                return Self::error_response(
                    id,
                    -32602,
                    "Either content or fileUrl must be provided.",
                );
            }
        };

        // Create chunks from content (simple line-based chunking)
        let lines: Vec<&str> = content_text.lines().collect();
        let chunks: Vec<String> = lines
            .chunks(10) // 10 lines per chunk
            .enumerate()
            .map(|(i, chunk)| {
                let start_line = i * 10 + 1;
                let _end_line = (start_line + chunk.len()).saturating_sub(1);
                chunk.join("\n")
            })
            .collect();

        // Determine file path and MIME type for storage
        let (mime_type, final_filename, final_size, _final_uploaded_at) = match (
            &args.content,
            &args.file_url,
        ) {
            (Some(_), None) => {
                // For direct content, use metadata or defaults
                let filename = filename.unwrap_or_else(|| "direct_content".to_string());
                let mime_type = mime_type_from_metadata.unwrap_or_else(|| "text/plain".to_string());
                let size = size_from_metadata.unwrap_or(content_text.len() as u64);
                let uploaded_at = uploaded_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                (mime_type, filename, size, uploaded_at)
            }
            (None, Some(file_url)) => {
                let file_path_str = extract_file_path_from_url(file_url).unwrap();
                // Use metadata if provided, otherwise determine from file extension
                let mime_type = mime_type_from_metadata.unwrap_or_else(|| {
                    match std::path::Path::new(&file_path_str).extension() {
                        Some(ext) => match ext.to_str().unwrap_or("").to_lowercase().as_str() {
                            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                            "pdf" => "application/pdf",
                            "txt" => "text/plain",
                            "md" => "text/markdown",
                            "csv" => "text/csv",
                            _ => "text/plain",
                        },
                        None => "text/plain",
                    }.to_string()
                });
                let filename = filename.unwrap_or_else(|| {
                    std::path::Path::new(&file_path_str)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown_file")
                        .to_string()
                });
                let size = size_from_metadata.unwrap_or(0); // File size from parsing
                let uploaded_at = uploaded_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                (mime_type, filename, size, uploaded_at)
            }
            _ => unreachable!("Already validated above"),
        };

        // Store the content
        let mut storage = self.storage.lock().await;
        let content_item = match storage
            .add_content(
                &args.store_id,
                &final_filename,
                &mime_type,
                final_size as usize,
                &content_text,
                chunks,
            )
            .await
        {
            Ok(item) => item,
            Err(e) => {
                return Self::error_response(id, -32603, &format!("Failed to store content: {e}"));
            }
        };

        // Create text chunks for search indexing
        let text_chunks: Vec<search::TextChunk> = lines
            .chunks(10)
            .enumerate()
            .map(|(i, chunk_lines)| {
                let start_line = i * 10 + 1;
                let end_line = start_line + chunk_lines.len().saturating_sub(1);
                search::TextChunk {
                    id: format!("chunk_{}_{}", content_item.id, i),
                    content_id: content_item.id.clone(),
                    text: chunk_lines.join("\n"),
                    line_range: (start_line, end_line),
                }
            })
            .collect();

        // Index chunks for search
        {
            let mut search_engine = self.search_engine.lock().await;
            if let Err(e) = search_engine.add_chunks(text_chunks).await {
                // Log error but don't fail the operation
                eprintln!("Warning: Failed to index content for search: {e}");
            }
        }

        Self::dual_response(
            id,
            &format!(
                "Content added successfully!\n\nFile: {}\nContent ID: {}\nStore ID: {}\nMIME Type: {}\nSize: {} bytes\nLine Count: {}\nChunks Created: {}\nUploaded: {}\n\nPreview:\n{}",
                content_item.filename,
                content_item.id,
                content_item.store_id,
                content_item.mime_type,
                content_item.size,
                content_item.line_count,
                content_item.chunk_count,
                content_item.uploaded_at,
                content_item.preview
            ),
            serde_json::json!({
                "storeId": content_item.store_id,
                "contentId": content_item.id,
                "filename": content_item.filename,
                "mimeType": content_item.mime_type,
                "size": content_item.size,
                "lineCount": content_item.line_count,
                "preview": content_item.preview,
                "uploadedAt": content_item.uploaded_at,
                "chunkCount": content_item.chunk_count
            }),
        )
    }

    async fn handle_list_content(&self, params: Value) -> MCPResponse {
        let id = Self::generate_request_id();
        let args: ListContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Self::error_response(
                    id,
                    -32602,
                    &format!("Invalid list_content parameters: {e}"),
                );
            }
        };

        let storage = self.storage.lock().await;
        let (contents, total) = match storage.list_content(&args.store_id, 0, 100).await {
            Ok((contents, total)) => (contents, total),
            Err(e) => {
                return Self::error_response(id, -32603, &format!("Failed to list content: {e}"));
            }
        };

        let content_list: Vec<serde_json::Value> = contents
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "contentId": item.id,
                    "storeId": item.store_id,
                    "filename": item.filename,
                    "mimeType": item.mime_type,
                    "size": item.size,
                    "lineCount": item.line_count,
                    "preview": item.preview,
                    "uploadedAt": item.uploaded_at,
                    "chunkCount": item.chunk_count,
                    "lastAccessedAt": item.last_accessed_at
                })
            })
            .collect();

        Self::dual_response(
            id,
            &format!(
                "Content listing for store:\n\nTotal items: {}\n\n{}",
                total,
                content_list
                    .iter()
                    .map(|item| format!(
                        "• {} (ID: {}, {} bytes, {} lines)",
                        item.get("filename")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown"),
                        item.get("contentId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown"),
                        item.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                        item.get("lineCount").and_then(|v| v.as_u64()).unwrap_or(0)
                    ))
                    .collect::<Vec<String>>()
                    .join("\n")
            ),
            serde_json::json!({
                "storeId": args.store_id,
                "contents": content_list,
                "total": total,
                "hasMore": false
            }),
        )
    }

    async fn handle_read_content(&self, params: Value) -> MCPResponse {
        let id = Self::generate_request_id();
        let args: ReadContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Self::error_response(
                    id,
                    -32602,
                    &format!("Invalid read_content parameters: {e}"),
                );
            }
        };

        let storage = self.storage.lock().await;
        let content = match storage
            .read_content(&args.content_id, args.from_line.unwrap_or(1), args.to_line)
            .await
        {
            Ok(content) => content,
            Err(e) => {
                return Self::error_response(id, -32603, &format!("Failed to read content: {e}"));
            }
        };

        Self::dual_response(
            id,
            &format!(
                "Content read successfully!\n\nContent ID: {}\nFrom Line: {}\nTo Line: {}\n\n--- Content ---\n{}",
                args.content_id,
                args.from_line.unwrap_or(1),
                args.to_line.map(|n| n.to_string()).unwrap_or("end".to_string()),
                content
            ),
            serde_json::json!({
                "content": content,
                "lineRange": [
                    args.from_line.unwrap_or(1),
                    args.to_line.unwrap_or_else(|| content.lines().count().max(1))
                ]
            }),
        )
    }

    async fn handle_keyword_search(&self, params: Value) -> MCPResponse {
        let id = Self::generate_request_id();
        let args: KeywordSearchArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Self::error_response(
                    id,
                    -32602,
                    &format!("Invalid keyword_search parameters: {e}"),
                );
            }
        };

        let search_engine = self.search_engine.lock().await;
        let results = match search_engine.search_bm25(&args.query, 10).await {
            Ok(results) => results,
            Err(e) => {
                return Self::error_response(id, -32603, &format!("Failed to search content: {e}"));
            }
        };

        let search_results: Vec<serde_json::Value> = results
            .into_iter()
            .map(|result| {
                serde_json::json!({
                    "contentId": result.content_id,
                    "chunkId": result.chunk_id,
                    "score": result.score,
                    "matchedText": result.matched_text,
                    "lineRange": result.line_range
                })
            })
            .collect();

        Self::dual_response(
            id,
            &format!(
                "Search completed!\n\nQuery: \"{}\"\nStore ID: {}\nResults found: {}\n\n{}",
                args.query,
                args.store_id,
                search_results.len(),
                if search_results.is_empty() {
                    "No results found for your search query.".to_string()
                } else {
                    search_results
                        .iter()
                        .map(|result| {
                            format!(
                                "📄 Content ID: {} (Score: {:.2})\n   Lines {}-{}: {}",
                                result
                                    .get("contentId")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown"),
                                result.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                result
                                    .get("lineRange")
                                    .and_then(|v| v.as_array())
                                    .and_then(|arr| arr.first())
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0),
                                result
                                    .get("lineRange")
                                    .and_then(|v| v.as_array())
                                    .and_then(|arr| arr.get(1))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0),
                                result
                                    .get("matchedText")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .trim()
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n\n")
                }
            ),
            serde_json::json!({
                "results": search_results
            }),
        )
    }
}

/// Helper: simple object schema from properties/required
fn object_schema(props: HashMap<String, JSONSchema>, required: Vec<String>) -> JSONSchema {
    crate::mcp::utils::schema_builder::object_schema(props, required)
}

/// Helper: string property
fn string_prop(desc: Option<&str>) -> JSONSchema {
    crate::mcp::utils::schema_builder::string_prop(None, None, desc)
}
