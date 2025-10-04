// handlers.rs - Tool handler implementations
use super::server::ContentStoreServer;
use super::types::*;
use super::{helpers, parsers, search};
use crate::mcp::MCPResponse;
use log::error;
use serde_json::Value;

impl ContentStoreServer {
    pub(crate) async fn handle_add_content(&self, params: Value) -> MCPResponse {
        let id = Self::generate_request_id();

        let args: AddContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Self::error_response(
                    id,
                    -32602,
                    &format!("Invalid add_content parameters: {e}"),
                );
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
            (Some(content), None) => content.clone(),
            (None, Some(file_url)) => {
                let file_path_str = match helpers::extract_file_path_from_url(file_url) {
                    Ok(path) => path,
                    Err(e) => {
                        return Self::error_response(id, -32602, &format!("Invalid file URL: {e}"));
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

        let session_id = match self.require_active_session(&id) {
            Ok(session_id) => session_id,
            Err(error_response) => return *error_response,
        };

        if let Err(e) = self.ensure_session_store(&session_id).await {
            error!("Failed to ensure content store for session {session_id}: {e}");
            return Self::error_response(
                id,
                -32603,
                &format!("Failed to prepare content store for session {session_id}: {e}"),
            );
        }

        // Create chunks from content (simple line-based chunking)
        let lines: Vec<&str> = content_text.lines().collect();
        let chunks: Vec<String> = helpers::create_text_chunks(&lines, 10);

        // Determine file path and MIME type for storage
        let (mime_type, final_filename, final_size, _final_uploaded_at) =
            match (&args.content, &args.file_url) {
                (Some(_), None) => {
                    // For direct content, use metadata or defaults
                    let filename = filename.unwrap_or_else(|| "direct_content".to_string());
                    let mime_type =
                        mime_type_from_metadata.unwrap_or_else(|| "text/plain".to_string());
                    let size = size_from_metadata.unwrap_or(content_text.len() as u64);
                    let uploaded_at =
                        uploaded_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                    (mime_type, filename, size, uploaded_at)
                }
                (None, Some(file_url)) => {
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
            serde_json::json!({
                "sessionId": content_item.session_id,
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

    pub(crate) async fn handle_list_content(&self, params: Value) -> MCPResponse {
        let id = Self::generate_request_id();
        let args: ListContentArgs = if params.is_null() {
            ListContentArgs { pagination: None }
        } else {
            match serde_json::from_value(params) {
                Ok(args) => args,
                Err(e) => {
                    return Self::error_response(
                        id,
                        -32602,
                        &format!("Invalid list_content parameters: {e}"),
                    );
                }
            }
        };

        let session_id = match self.require_active_session(&id) {
            Ok(session_id) => session_id,
            Err(error_response) => return *error_response,
        };

        if let Err(e) = self.ensure_session_store(&session_id).await {
            error!(
                "Failed to ensure content store for session {session_id} while listing content: {e}"
            );
            return Self::error_response(
                id,
                -32603,
                &format!("Failed to prepare content store for session {session_id}: {e}"),
            );
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
                return Self::error_response(id, -32603, &format!("Failed to list content: {e}"));
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
                "sessionId": session_id,
                "contents": content_list,
                "total": total,
                "hasMore": false
            }),
        )
    }

    pub(crate) async fn handle_read_content(&self, params: Value) -> MCPResponse {
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

    pub(crate) async fn handle_keyword_search(&self, params: Value) -> MCPResponse {
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

        let session_id = match self.require_active_session(&id) {
            Ok(session_id) => session_id,
            Err(error_response) => return *error_response,
        };

        if let Err(e) = self.ensure_session_store(&session_id).await {
            error!(
                "Failed to ensure content store for session {session_id} during keyword search: {e}"
            );
            return Self::error_response(
                id,
                -32603,
                &format!("Failed to prepare content store for session {session_id}: {e}"),
            );
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
                return Self::error_response(id, -32603, &format!("Failed to search content: {e}"));
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

        Self::dual_response(
            id,
            &format!(
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
            serde_json::json!({
                    "sessionId": session_id,
                "results": search_results
            }),
        )
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
        let session_manager = Arc::new(SessionManager::new().unwrap());
        let server = ContentStoreServer::new(session_manager);
        (server, temp_dir)
    }
    #[tokio::test]
    async fn test_handle_add_content_direct_content() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: None,
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

        let response = server.handle_add_content(params).await;

        // Verify response
        assert!(response.error.is_none());
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        let structured_content = &result["structuredContent"];
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

        let response = server.handle_add_content(params).await;

        // Should return error about missing session
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32002);
    }

    #[tokio::test]
    async fn test_handle_add_content_both_content_and_url() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: None,
            })
            .await
            .unwrap();

        let params = serde_json::json!({
            "content": "Test content",
            "file_url": "file:///test.txt"
        });

        let response = server.handle_add_content(params).await;

        // Should return error about ambiguous input
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32602);
    }

    #[tokio::test]
    async fn test_handle_list_content_empty() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: None,
            })
            .await
            .unwrap();

        let params = serde_json::json!({});
        let response = server.handle_list_content(params).await;

        assert!(response.error.is_none());
        let result = response.result.unwrap();
        let structured_content = &result["structuredContent"];
        assert_eq!(structured_content["total"], 0);
        assert_eq!(structured_content["contents"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_handle_keyword_search_no_results() {
        let (server, _temp) = setup_test_server().await;

        server
            .switch_context(ServiceContextOptions {
                session_id: Some("test-session".to_string()),
                assistant_id: None,
            })
            .await
            .unwrap();

        let params = serde_json::json!({
            "query": "nonexistent",
            "options": {
                "top_n": 5
            }
        });

        let response = server.handle_keyword_search(params).await;

        assert!(response.error.is_none());
        let result = response.result.unwrap();
        let structured_content = &result["structuredContent"];
        assert_eq!(structured_content["results"].as_array().unwrap().len(), 0);
    }
}
