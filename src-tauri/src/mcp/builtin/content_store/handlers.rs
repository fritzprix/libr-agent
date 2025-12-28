use super::server::ContentStoreServer;
use super::types::*;
use super::{helpers, parsers, search};
use crate::mcp::types::MCPResult;
use log::error;
use serde_json::Value;

impl ContentStoreServer {
    pub(crate) async fn handle_add_content(&self, params: Value) -> Result<MCPResult, String> {
        let args: AddContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(MCPResult::error(&format!(
                    "Invalid add_content parameters: {e}"
                )));
            }
        };

        // Extract metadata
        let metadata = args.metadata.as_ref();
        let filename = metadata.and_then(|m| m.filename.clone());
        let mime_type_from_metadata = metadata.and_then(|m| m.mime_type.clone());
        let size_from_metadata = metadata.and_then(|m| m.size);
        let uploaded_at = metadata.and_then(|m| m.uploaded_at.clone());

        // Validate input
        let content_text = match (&args.content, &args.file_url) {
            (Some(content), Option::None) => content.clone(),
            (Option::None, Some(file_url)) => {
                let file_path_str = match helpers::extract_file_path_from_url(file_url) {
                    Ok(path) => path,
                    Err(e) => {
                        return Ok(MCPResult::error(&format!("Invalid file URL: {e}")));
                    }
                };

                let mime_type =
                    helpers::mime_type_from_extension(std::path::Path::new(&file_path_str));

                // Parse file
                match parsers::DocumentParser::parse_file(
                    std::path::Path::new(&file_path_str),
                    mime_type,
                )
                .await
                {
                    parsers::ParseResult::Text(content) => content,
                    parsers::ParseResult::Error(e) => {
                        return Ok(MCPResult::error(&format!(
                            "Failed to parse file {file_path_str}: {e}"
                        )));
                    }
                }
            }
            (Some(_), Some(_)) => {
                return Ok(MCPResult::error(
                    "Cannot provide both content and fileUrl. Choose one.",
                ));
            }
            (Option::None, Option::None) => {
                return Ok(MCPResult::error(
                    "Either content or fileUrl must be provided.",
                ));
            }
        };

        let session_id = match self.require_active_session_result() {
            Ok(session_id) => session_id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        if let Err(e) = self.ensure_session_store(&session_id).await {
            error!("Failed to ensure content store for session {session_id}: {e}");
            return Ok(MCPResult::error(&format!(
                "Failed to prepare content store for session {session_id}: {e}"
            )));
        }

        // Create chunks from content (simple line-based chunking)
        let lines: Vec<&str> = content_text.lines().collect();
        let chunks: Vec<String> = helpers::create_text_chunks(&lines, 10);

        // Determine file path and MIME type for storage
        let (mime_type, final_filename, final_size, _final_uploaded_at) =
            match (&args.content, &args.file_url) {
                (Some(_), Option::None) => {
                    // For direct content, use metadata or defaults
                    let filename = filename.unwrap_or_else(|| "direct_content".to_string());
                    let mime_type =
                        mime_type_from_metadata.unwrap_or_else(|| "text/plain".to_string());
                    let size = size_from_metadata.unwrap_or(content_text.len() as u64);
                    let uploaded_at =
                        uploaded_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                    (mime_type, filename, size, uploaded_at)
                }
                (Option::None, Some(file_url)) => {
                    let file_path_str = helpers::extract_file_path_from_url(file_url).unwrap();
                    // Use metadata if provided, otherwise determine from file extension
                    let mime_type = mime_type_from_metadata.unwrap_or_else(|| {
                        helpers::mime_type_from_extension(std::path::Path::new(&file_path_str))
                            .to_string()
                    });
                    let filename = filename.unwrap_or_else(|| {
                        std::path::Path::new(&file_path_str)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown_file")
                            .to_string()
                    });
                    let size = size_from_metadata.unwrap_or(0); // File size from parsing
                    let uploaded_at =
                        uploaded_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                    (mime_type, filename, size, uploaded_at)
                }
                _ => unreachable!("Already validated above"),
            };

        // Store the content
        let mut storage = self.storage.lock().await;
        let content_item = match storage
            .add_content(
                &session_id,
                &final_filename,
                &mime_type,
                final_size as usize,
                &content_text,
                chunks,
                args.src_url.clone(),
            )
            .await
        {
            Ok(item) => item,
            Err(e) => {
                return Ok(MCPResult::error(&format!("Failed to store content: {e}")));
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

        Ok(MCPResult {
            content: Some(vec![crate::mcp::types::MCPContent::Text {
                text: format!(
                    "Content added successfully!\n\nFile: {}\nContent ID: {}\nSession ID: {}\nMIME Type: {}\nSize: {} bytes\nLine Count: {}\nChunks Created: {}\nUploaded: {}\n\nPreview:\n{}",
                    content_item.filename,
                    content_item.id,
                    content_item.session_id,
                    content_item.mime_type,
                    content_item.size,
                    content_item.line_count,
                    content_item.chunk_count,
                    content_item.uploaded_at,
                    content_item.preview
                ),
            }]),
            structured_content: Some(serde_json::json!({
                "sessionId": content_item.session_id,
                "contentId": content_item.id,
                "filename": content_item.filename,
                "mimeType": content_item.mime_type,
                "size": content_item.size,
                "lineCount": content_item.line_count,
                "preview": content_item.preview,
                "uploadedAt": content_item.uploaded_at,
                "chunkCount": content_item.chunk_count
            })),
            is_error: Some(false),
        })
    }

    pub(crate) async fn handle_list_content(&self, params: Value) -> Result<MCPResult, String> {
        let args: ListContentArgs = if params.is_null() {
            ListContentArgs {
                pagination: Option::None,
            }
        } else {
            match serde_json::from_value(params) {
                Ok(args) => args,
                Err(e) => {
                    return Ok(MCPResult::error(&format!(
                        "Invalid list_content parameters: {e}"
                    )));
                }
            }
        };

        let session_id = match self.require_active_session_result() {
            Ok(id) => id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        if let Err(e) = self.ensure_session_store(&session_id).await {
            error!(
                "Failed to ensure content store for session {session_id} while listing content: {e}"
            );
            return Ok(MCPResult::error(&format!(
                "Failed to prepare content store for session {session_id}: {e}"
            )));
        }

        let (offset, limit) = args.pagination.as_ref().map_or((0usize, 100usize), |p| {
            let offset = p.offset.unwrap_or(0);
            let limit = p.limit.unwrap_or(100).clamp(1, 1000);
            (offset, limit)
        });

        let storage = self.storage.lock().await;
        let (contents, total) = match storage.list_content(&session_id, offset, limit).await {
            Ok((contents, total)) => (contents, total),
            Err(e) => {
                return Ok(MCPResult::error(&format!("Failed to list content: {e}")));
            }
        };

        let content_list: Vec<serde_json::Value> = contents
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "contentId": item.id,
                    "sessionId": item.session_id,
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

        Ok(MCPResult {
            content: Some(vec![crate::mcp::types::MCPContent::Text {
                text: format!(
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
            }]),
            structured_content: Some(serde_json::json!({
                "sessionId": session_id,
                "contents": content_list,
                "total": total,
                "hasMore": false
            })),
            is_error: Some(false),
        })
    }

    pub(crate) async fn handle_read_content(&self, params: Value) -> Result<MCPResult, String> {
        let args: ReadContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(MCPResult::error(&format!(
                    "Invalid read_content parameters: {e}"
                )));
            }
        };

        // Get current session ID
        let session_id = match self.require_active_session_result() {
            Ok(id) => id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        // Verify content belongs to current session
        let content_session_id = {
            let storage = self.storage.lock().await;
            storage
                .get_content_session_id(&args.content_id)
                .ok_or_else(|| format!("Content '{}' not found", args.content_id))
        };

        let content_session_id = match content_session_id {
            Ok(id) => id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        if content_session_id != session_id {
            return Ok(MCPResult::error(&format!(
                "Access denied: Content '{}' belongs to a different session",
                args.content_id
            )));
        }

        // Read content (session verification passed)
        let storage = self.storage.lock().await;
        let content = match storage
            .read_content(&args.content_id, args.from_line.unwrap_or(1), args.to_line)
            .await
        {
            Ok(content) => content,
            Err(e) => {
                return Ok(MCPResult::error(&format!("Failed to read content: {e}")));
            }
        };

        Ok(MCPResult {
            content: Some(vec![crate::mcp::types::MCPContent::Text {
                text: format!(
                    "Content read successfully!\n\nContent ID: {}\nFrom Line: {}\nTo Line: {}\n\n--- Content ---\n{}",
                    args.content_id,
                    args.from_line.unwrap_or(1),
                    args.to_line.map(|n| n.to_string()).unwrap_or("end".to_string()),
                    content
                ),
            }]),
            structured_content: Some(serde_json::json!({
                "content": content,
                "lineRange": [
                    args.from_line.unwrap_or(1),
                    args.to_line.unwrap_or_else(|| content.lines().count().max(1))
                ]
            })),
            is_error: Some(false),
        })
    }

    pub(crate) async fn handle_keyword_search(&self, params: Value) -> Result<MCPResult, String> {
        let args: KeywordSearchArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(MCPResult::error(&format!(
                    "Invalid keyword_search parameters: {e}"
                )));
            }
        };

        let session_id = match self.require_active_session_result() {
            Ok(id) => id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        if let Err(e) = self.ensure_session_store(&session_id).await {
            error!(
                "Failed to ensure content store for session {session_id} during keyword search: {e}"
            );
            return Ok(MCPResult::error(&format!(
                "Failed to prepare content store for session {session_id}: {e}"
            )));
        }

        let top_n = args
            .options
            .as_ref()
            .and_then(|opts| opts.top_n)
            .unwrap_or(10)
            .clamp(1, 100);

        let ranking_limit = std::cmp::max(top_n, 50);
        let score_threshold = args.options.as_ref().and_then(|opts| opts.threshold);

        let search_engine = self.search_engine.lock().await;
        let all_results = match search_engine.search_bm25(&args.query, ranking_limit).await {
            Ok(results) => results,
            Err(e) => {
                return Ok(MCPResult::error(&format!("Failed to search content: {e}")));
            }
        };

        // Filter results by session_id
        let storage = self.storage.lock().await;
        let mut filtered_results = Vec::new();
        for result in all_results {
            let belongs_to_session = storage
                .get_content_session_id(&result.content_id)
                .map(|sid| sid == session_id)
                .unwrap_or(false);

            if !belongs_to_session {
                continue;
            }

            if let Some(threshold) = score_threshold {
                if result.score < threshold {
                    continue;
                }
            }

            filtered_results.push(result);

            if filtered_results.len() >= top_n {
                break;
            }
        }

        let search_results: Vec<serde_json::Value> = filtered_results
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

        Ok(MCPResult {
            content: Some(vec![crate::mcp::types::MCPContent::Text {
                text: format!(
                    "Search completed!\n\nQuery: \"{}\"\nSession ID: {}\nResults found: {}\n\n{}",
                    args.query,
                    session_id,
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
            }]),
            structured_content: Some(serde_json::json!({
                "sessionId": session_id,
                "results": search_results
            })),
            is_error: Some(false),
        })
    }

    pub(crate) async fn handle_delete_content(&self, params: Value) -> Result<MCPResult, String> {
        let args: DeleteContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(MCPResult::error(&format!(
                    "Invalid delete_content parameters: {e}"
                )));
            }
        };

        // Get current session ID from context
        let session_id = match self.require_active_session_result() {
            Ok(id) => id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        // Verify the content belongs to the current session
        let content_session_id = {
            let storage = self.storage.lock().await;
            if let Some(sid) = storage.get_content_session_id(&args.content_id) {
                sid
            } else {
                return Ok(MCPResult::error(&format!(
                    "Content '{}' not found",
                    args.content_id
                )));
            }
        };

        if content_session_id != session_id {
            return Ok(MCPResult::error(&format!(
                "Content '{}' does not belong to current session",
                args.content_id
            )));
        }

        // Delete from storage
        let mut storage = self.storage.lock().await;
        if let Err(e) = storage.delete_content(&args.content_id).await {
            return Ok(MCPResult::error(&format!("Failed to delete content: {e}")));
        }

        // Remove from search index
        let mut search_engine = self.search_engine.lock().await;
        if let Err(e) = search_engine.remove_chunks(&args.content_id).await {
            // Log error but don't fail the operation since content is already deleted
            error!("Failed to remove content from search index: {e}");
        }

        Ok(MCPResult {
            content: Some(vec![crate::mcp::types::MCPContent::Text {
                text: format!("Content '{}' deleted successfully", args.content_id),
            }]),
            structured_content: Some(serde_json::json!({
                "contentId": args.content_id,
                "sessionId": session_id
            })),
            is_error: Some(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::ServiceContextOptions;
    use crate::session::SessionManager;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn setup_test_server() -> (ContentStoreServer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let session_manager =
            Arc::new(SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).unwrap());
        let server = ContentStoreServer::new(session_manager);
        (server, temp_dir)
    }
    #[tokio::test]
    async fn test_handle_add_content_direct_content() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: Option::None,
            })
            .await
            .unwrap();

        let params = serde_json::json!({
            "content": "Test content\nLine 2\nLine 3",
            "metadata": {
                "filename": "test.txt",
                "mime_type": "text/plain"
            }
        });

        let result = server.handle_add_content(params).await.unwrap();

        // Verify response
        assert_eq!(result.is_error, Some(false));
        assert!(result.structured_content.is_some());

        let structured_content = result.structured_content.unwrap();
        assert_eq!(structured_content["filename"], "test.txt");
        assert_eq!(structured_content["mimeType"], "text/plain");
    }

    #[tokio::test]
    async fn test_handle_add_content_missing_session() {
        let (server, _temp) = setup_test_server().await;

        // Don't setup session context
        let params = serde_json::json!({
            "content": "Test content"
        });

        let result = server.handle_add_content(params).await.unwrap();

        // Should return error about missing session
        assert_eq!(result.is_error, Some(true));
        assert!(result.content.is_some());
    }

    #[tokio::test]
    async fn test_handle_add_content_both_content_and_url() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: Option::None,
            })
            .await
            .unwrap();

        let params = serde_json::json!({
            "content": "Test content",
            "fileUrl": "file:///test.txt"
        });

        let result = server.handle_add_content(params).await.unwrap();

        // Should return error about ambiguous input
        assert_eq!(result.is_error, Some(true));
        assert!(result.content.is_some());
    }

    #[tokio::test]
    async fn test_handle_list_content_empty() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: Option::None,
            })
            .await
            .unwrap();

        let params = serde_json::json!({});
        let result = server.handle_list_content(params).await.unwrap();

        assert_eq!(result.is_error, Some(false));
        let structured_content = result.structured_content.unwrap();
        assert_eq!(structured_content["total"], 0);
        assert_eq!(structured_content["contents"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_handle_keyword_search_no_results() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: Option::None,
            })
            .await
            .unwrap();

        let params = serde_json::json!({
            "query": "nonexistent",
            "options": {
                "top_n": 5
            }
        });

        let result = server.handle_keyword_search(params).await.unwrap();

        assert_eq!(result.is_error, Some(false));
        let structured_content = result.structured_content.unwrap();
        assert_eq!(structured_content["results"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_handle_delete_content() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: Option::None,
            })
            .await
            .unwrap();

        // Add content first
        let add_params = serde_json::json!({
            "content": "Test content to delete",
            "metadata": {
                "filename": "delete_test.txt"
            }
        });
        let add_result = server.handle_add_content(add_params).await.unwrap();
        let content_id = add_result.structured_content.unwrap()["contentId"]
            .as_str()
            .unwrap()
            .to_string();

        // Delete content
        let delete_params = serde_json::json!({
            "contentId": content_id
        });

        // This would hang if deadlock exists
        let result = server.handle_delete_content(delete_params).await.unwrap();

        assert_eq!(result.is_error, Some(false));

        // Verify deletion
        let list_params = serde_json::json!({});
        let list_result = server.handle_list_content(list_params).await.unwrap();
        let total = list_result.structured_content.unwrap()["total"]
            .as_u64()
            .unwrap();
        assert_eq!(total, 0);
    }
}
