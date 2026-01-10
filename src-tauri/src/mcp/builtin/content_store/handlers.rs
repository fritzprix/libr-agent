use super::server::ContentStoreServer;
use super::types::*;
use super::{helpers, parsers, search};
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, not_found_error, operation_failed_error, SuccessHint,
    ToolGroup,
};
use crate::mcp::types::MCPResult;
use log::error;
use serde_json::Value;

impl ContentStoreServer {
    pub(crate) async fn handle_save_knowledge(&self, params: Value) -> Result<MCPResult, String> {
        let args: AddContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(invalid_input_error(
                    &format!("Invalid save_knowledge parameters: {e}"),
                    ToolGroup::ContentStore,
                ));
            }
        };

        // Extract metadata
        let metadata = args.metadata.as_ref();
        // Map title to filename if filename is missing
        let filename = metadata
            .and_then(|m| m.filename.clone())
            .or(args.title.clone());

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
                        return Ok(invalid_input_error(
                            &format!("Invalid file URL: {e}"),
                            ToolGroup::ContentStore,
                        ));
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
                        return Ok(operation_failed_error(
                            "Parse file",
                            &format!("{file_path_str}: {e}"),
                            vec![
                                "Ensure the file format is supported (PDF, HTML, markdown, code)"
                                    .to_string(),
                                "Check the file is not corrupted".to_string(),
                                "Try providing content directly instead of fileUrl".to_string(),
                            ],
                            ToolGroup::ContentStore,
                        ));
                    }
                }
            }
            (Some(_), Some(_)) => {
                return Ok(invalid_input_error(
                    "Cannot provide both content and fileUrl. Choose one.",
                    ToolGroup::ContentStore,
                ));
            }
            (Option::None, Option::None) => {
                return Ok(missing_param_error(
                    "content or fileUrl",
                    ToolGroup::ContentStore,
                ));
            }
        };

        let session_id = match self.require_active_session_result() {
            Ok(session_id) => session_id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        if let Err(e) = self.ensure_session_store(&session_id).await {
            error!("Failed to ensure content store for session {session_id}: {e}");
            return Ok(operation_failed_error(
                "Prepare content store",
                &format!("session {session_id}: {e}"),
                vec![
                    "Check database connectivity".to_string(),
                    "Ensure session is active".to_string(),
                    "Retry the operation".to_string(),
                ],
                ToolGroup::ContentStore,
            ));
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
                return Ok(operation_failed_error(
                    "Store content",
                    &e.to_string(),
                    vec![
                        "Check database connectivity".to_string(),
                        "Verify content format is valid".to_string(),
                        "Try with smaller content size".to_string(),
                    ],
                    ToolGroup::ContentStore,
                ));
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

        // Track this upload for service context
        {
            let mut recent = self.recent_uploads.lock().await;

            // Add to front of queue (most recent first)
            recent.push_front(super::server::RecentUploadInfo {
                content_id: content_item.id.clone(),
                filename: content_item.filename.clone(),
                mime_type: content_item.mime_type.clone(),
                line_count: content_item.line_count,
                uploaded_at: content_item.uploaded_at.clone(),
            });

            // Keep only last 10 uploads
            if recent.len() > 10 {
                recent.pop_back();
            }
        }

        let hint = SuccessHint::new(
            format!(
                "Content saved successfully\n  ID: {}\n  Title: {}\n  Size: {} bytes, {} lines\n  Preview: {}",
                content_item.id,
                content_item.filename,
                content_item.size,
                content_item.line_count,
                if content_item.preview.len() > 100 {
                    let truncated: String = content_item.preview.chars().take(100).collect();
                    format!("{}...", truncated)
                } else {
                    content_item.preview.clone()
                }
            ),
            vec![
                format!("Use readContent with contentId='{}' to view the full content", content_item.id),
                "Use keywordSimilaritySearch to find content by keywords".to_string(),
            ],
        );

        Ok(hint.to_mcp_result_with_data(Some(serde_json::json!({
            "sessionId": content_item.session_id,
            "contentId": content_item.id,
            "filename": content_item.filename,
            "mimeType": content_item.mime_type,
            "size": content_item.size,
            "lineCount": content_item.line_count,
            "preview": content_item.preview,
            "uploadedAt": content_item.uploaded_at,
            "chunkCount": content_item.chunk_count
        }))))
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
                    return Ok(invalid_input_error(
                        &format!("Invalid list_content parameters: {e}"),
                        ToolGroup::ContentStore,
                    ));
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
            return Ok(operation_failed_error(
                "Prepare content store",
                &format!("session {session_id}: {e}"),
                vec![
                    "Check database connectivity".to_string(),
                    "Ensure session is active".to_string(),
                    "Retry the operation".to_string(),
                ],
                ToolGroup::ContentStore,
            ));
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
                return Ok(operation_failed_error(
                    "List content",
                    &e.to_string(),
                    vec![
                        "Check database connectivity".to_string(),
                        "Verify session is active".to_string(),
                        "Retry the operation".to_string(),
                    ],
                    ToolGroup::ContentStore,
                ));
            }
        };

        // Build detailed content list for agent visibility FIRST (before consuming contents)
        let content_details: Vec<String> = contents
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let preview_text = if item.preview.len() > 80 {
                    let truncated: String = item.preview.chars().take(80).collect();
                    format!("{}...", truncated)
                } else {
                    item.preview.clone()
                };
                format!(
                    "[{}] ID: {}\n    Title: {}\n    Size: {} bytes, {} lines\n    Line Range: 1-{}\n    Preview: {}\n    Created: {}",
                    idx + 1,
                    item.id,
                    item.filename,
                    item.size,
                    item.line_count,
                    item.line_count,
                    preview_text,
                    item.uploaded_at
                )
            })
            .collect();

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

        let has_more = offset + content_list.len() < total;

        let items_text = if content_details.is_empty() {
            String::new()
        } else {
            format!("\n\n{}", content_details.join("\n\n"))
        };

        let hint = SuccessHint::new(
            format!(
                "Found {} of {} content items{}",
                content_list.len(),
                total,
                items_text
            ),
            if has_more {
                vec![format!(
                    "Use pagination.offset={} to see more items",
                    offset + limit
                )]
            } else if total > 0 {
                vec![
                    "Use readContent with a contentId from above to view full contents".to_string(),
                ]
            } else {
                vec!["Use addContent to add files to the content store".to_string()]
            },
        );

        Ok(hint.to_mcp_result_with_data(Some(serde_json::json!({
            "sessionId": session_id,
            "contents": content_list,
            "total": total,
            "hasMore": has_more
        }))))
    }

    pub(crate) async fn handle_read_content(&self, params: Value) -> Result<MCPResult, String> {
        let args: ReadContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(invalid_input_error(
                    &format!("Invalid read_content parameters: {e}"),
                    ToolGroup::ContentStore,
                ));
            }
        };

        // Normalize content ID (add "content_" prefix if missing)
        let normalized_content_id = ContentStoreServer::normalize_content_id(&args.content_id);

        // Get current session ID
        let session_id = match self.require_active_session_result() {
            Ok(id) => id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        // Verify content belongs to current session
        let content_session_id = {
            let storage = self.storage.lock().await;
            storage
                .get_content_session_id(&normalized_content_id)
                .ok_or_else(|| format!("Content '{}' not found", args.content_id))
        };

        let content_session_id = match content_session_id {
            Ok(id) => id,
            Err(_) => {
                return Ok(not_found_error(
                    "Content",
                    &args.content_id,
                    ToolGroup::ContentStore,
                ))
            }
        };

        if content_session_id != session_id {
            return Ok(operation_failed_error(
                "Read content",
                &format!(
                    "Content '{}' belongs to a different session",
                    args.content_id
                ),
                vec![
                    "Use listContent to see content in current session".to_string(),
                    "Switch to the session that owns this content".to_string(),
                    "Verify the content ID is correct".to_string(),
                ],
                ToolGroup::ContentStore,
            ));
        }

        // Read content (session verification passed)
        let storage = self.storage.lock().await;

        // Get content metadata for accurate truncation messaging
        let content_item = storage
            .get_content_item(&normalized_content_id)
            .ok_or_else(|| format!("Content '{}' not found", args.content_id))?;
        let total_lines = content_item.line_count;

        let content = match storage
            .read_content(
                &normalized_content_id,
                args.from_line.unwrap_or(1),
                args.to_line,
            )
            .await
        {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read content",
                    &e.to_string(),
                    vec![
                        "Verify the content ID is correct".to_string(),
                        "Check line range is valid".to_string(),
                        "Use listContent to see available content".to_string(),
                    ],
                    ToolGroup::ContentStore,
                ));
            }
        };
        drop(storage);

        let from_line = args.from_line.unwrap_or(1);
        let to_line = args
            .to_line
            .unwrap_or_else(|| content.lines().count().max(1));

        // Determine if all requested lines were returned
        let is_fully_returned = to_line >= total_lines;
        let is_preview_truncated = content.len() > 2000;

        // Create appropriate truncation message
        let (content_preview, next_step_hint) = if is_preview_truncated {
            if is_fully_returned {
                // All lines returned, but preview is truncated for display
                (
                    format!(
                        "{}\n(Preview truncated for display. Full content in structured data. End of file reached - {} lines total)",
                        content.chars().take(2000).collect::<String>(),
                        total_lines
                    ),
                    None
                )
            } else {
                // Partial file, more lines available
                let remaining = total_lines.saturating_sub(to_line);
                (
                    format!(
                        "{}\n(Preview truncated. {} more lines remaining, {} lines total)",
                        content.chars().take(2000).collect::<String>(),
                        remaining,
                        total_lines
                    ),
                    Some(format!(
                        "To read more, use readContent with fromLine={}",
                        to_line + 1
                    )),
                )
            }
        } else if is_fully_returned {
            (format!("{}\n(End of file reached)", content), None)
        } else {
            (content.clone(), None)
        };

        let mut hints = vec![
            "Use keywordSimilaritySearch to find specific content".to_string(),
            format!(
                "Use deleteContent with contentId='{}' to remove this content",
                args.content_id
            ),
        ];

        if let Some(hint) = next_step_hint {
            hints.insert(0, hint);
        }

        let hint = SuccessHint::new(
            format!(
                "Content '{}' (lines {}-{}):\n\n{}",
                args.content_id, from_line, to_line, content_preview
            ),
            hints,
        );

        Ok(hint.to_mcp_result_with_data(Some(serde_json::json!({
            "content": content,
            "lineRange": [from_line, to_line],
            "totalLines": total_lines,
            "isComplete": is_fully_returned,
            "remainingLines": total_lines.saturating_sub(to_line),
            "suggestedNextRange": if is_fully_returned {
                serde_json::Value::Null
            } else {
                serde_json::json!([to_line + 1, total_lines.min(to_line + 100)])
            }
        }))))
    }

    pub(crate) async fn handle_search_knowledge(&self, params: Value) -> Result<MCPResult, String> {
        let args: KeywordSearchArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(invalid_input_error(
                    &format!("Invalid keyword_search parameters: {e}"),
                    ToolGroup::ContentStore,
                ));
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
            return Ok(operation_failed_error(
                "Prepare content store",
                &format!("session {session_id}: {e}"),
                vec![
                    "Check database connectivity".to_string(),
                    "Ensure session is active".to_string(),
                    "Retry the operation".to_string(),
                ],
                ToolGroup::ContentStore,
            ));
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
                return Ok(operation_failed_error(
                    "Search content",
                    &e.to_string(),
                    vec![
                        "Verify the search query is valid".to_string(),
                        "Check if content has been indexed".to_string(),
                        "Use listContent to see available content".to_string(),
                    ],
                    ToolGroup::ContentStore,
                ));
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

        // Build detailed results for agent visibility
        let results_text = if search_results.is_empty() {
            String::new()
        } else {
            let details: Vec<String> = search_results
                .iter()
                .enumerate()
                .map(|(idx, result)| {
                    format!(
                        "[{}] Content ID: {}\n    Score: {:.2}\n    Lines: {:?}\n    Match: {}",
                        idx + 1,
                        result["contentId"].as_str().unwrap_or("unknown"),
                        result["score"].as_f64().unwrap_or(0.0),
                        result["lineRange"],
                        result["matchedText"].as_str().unwrap_or("")
                    )
                })
                .collect();
            format!("\n\n{}", details.join("\n\n"))
        };

        let hint = SuccessHint::new(
            format!(
                "Search '{}' found {} results{}",
                args.query,
                search_results.len(),
                results_text
            ),
            if search_results.is_empty() {
                vec![
                    "Try different search keywords".to_string(),
                    "Use listContent to see all available content".to_string(),
                ]
            } else {
                vec!["Use readContent with a contentId from above to view full content".to_string()]
            },
        );

        Ok(hint.to_mcp_result_with_data(Some(serde_json::json!({
            "sessionId": session_id,
            "results": search_results
        }))))
    }

    pub(crate) async fn handle_delete_content(&self, params: Value) -> Result<MCPResult, String> {
        let args: DeleteContentArgs = match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(invalid_input_error(
                    &format!("Invalid delete_content parameters: {e}"),
                    ToolGroup::ContentStore,
                ));
            }
        };

        // Normalize content ID (add "content_" prefix if missing)
        let normalized_content_id = ContentStoreServer::normalize_content_id(&args.content_id);

        // Get current session ID from context
        let session_id = match self.require_active_session_result() {
            Ok(id) => id,
            Err(e) => return Ok(MCPResult::error(&e)),
        };

        // Verify the content belongs to the current session
        let content_session_id = {
            let storage = self.storage.lock().await;
            if let Some(sid) = storage.get_content_session_id(&normalized_content_id) {
                sid
            } else {
                return Ok(not_found_error(
                    "Content",
                    &args.content_id,
                    ToolGroup::ContentStore,
                ));
            }
        };

        if content_session_id != session_id {
            return Ok(operation_failed_error(
                "Delete content",
                &format!(
                    "Content '{}' belongs to a different session",
                    args.content_id
                ),
                vec![
                    "Use listContent to see content in current session".to_string(),
                    "Switch to the session that owns this content".to_string(),
                    "Verify the content ID is correct".to_string(),
                ],
                ToolGroup::ContentStore,
            ));
        }

        // Delete from storage
        let mut storage = self.storage.lock().await;
        if let Err(e) = storage.delete_content(&normalized_content_id).await {
            return Ok(operation_failed_error(
                "Delete content",
                &e.to_string(),
                vec![
                    "Check database connectivity".to_string(),
                    "Verify the content ID is correct".to_string(),
                    "Use listContent to see available content".to_string(),
                ],
                ToolGroup::ContentStore,
            ));
        }

        // Remove from search index
        let mut search_engine = self.search_engine.lock().await;
        if let Err(e) = search_engine.remove_chunks(&normalized_content_id).await {
            // Log error but don't fail the operation since content is already deleted
            error!("Failed to remove content from search index: {e}");
        }

        let hint = SuccessHint::new(
            format!("Content '{}' deleted successfully", args.content_id),
            vec!["Use listContent to see remaining content".to_string()],
        );

        Ok(hint.to_mcp_result_with_data(Some(serde_json::json!({
            "contentId": args.content_id,
            "sessionId": session_id
        }))))
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
        let server = ContentStoreServer::new("test-session".to_string(), session_manager);
        (server, temp_dir)
    }
    #[tokio::test]
    async fn test_handle_save_knowledge_direct_content() {
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

        let result = server.handle_save_knowledge(params).await.unwrap();

        // Verify response
        assert_eq!(result.is_error, Some(false));
        assert!(result.structured_content.is_some());

        let structured_content = result.structured_content.unwrap();
        assert_eq!(structured_content["filename"], "test.txt");
        assert_eq!(structured_content["mimeType"], "text/plain");
    }

    #[tokio::test]
    async fn test_handle_save_knowledge_missing_session() {
        let (server, _temp) = setup_test_server().await;

        // Don't setup session context - server will use default session_id
        let params = serde_json::json!({
            "content": "Test content"
        });

        let result = server.handle_save_knowledge(params).await.unwrap();

        // Should succeed and auto-create store for the session
        assert_eq!(result.is_error, Some(false));
        assert!(result.content.is_some());
    }

    #[tokio::test]
    async fn test_handle_save_knowledge_both_content_and_url() {
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

        let result = server.handle_save_knowledge(params).await.unwrap();

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
    async fn test_handle_search_knowledge_no_results() {
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

        let result = server.handle_search_knowledge(params).await.unwrap();

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
        let add_result = server.handle_save_knowledge(add_params).await.unwrap();
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
