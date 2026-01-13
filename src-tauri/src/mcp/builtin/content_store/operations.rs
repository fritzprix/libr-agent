use super::server::ContentStoreServer;
use super::types::*;
use super::{helpers, parsers, search};
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, operation_failed_error, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use log::error;
use serde_json::Value;

pub async fn save_knowledge(
    server: &ContentStoreServer,
    params: Value,
    session_id: &str,
) -> Result<MCPResult, String> {
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

            let mime_type = helpers::mime_type_from_extension(std::path::Path::new(&file_path_str));

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

    // Use passed session_id
    if let Err(e) = server.ensure_session_store(session_id).await {
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
                let mime_type = mime_type_from_metadata.unwrap_or_else(|| "text/plain".to_string());
                let size = size_from_metadata.unwrap_or(content_text.len() as u64);
                let uploaded_at = uploaded_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
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
                let uploaded_at = uploaded_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                (mime_type, filename, size, uploaded_at)
            }
            _ => unreachable!("Already validated above"),
        };

    // Store the content
    let mut storage = server.storage.lock().await;
    let content_item = match storage
        .add_content(
            session_id,
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
        match server.get_search_engine(session_id).await {
            Ok(engine_arc) => {
                let mut search_engine = engine_arc.lock().await;
                if let Err(e) = search_engine.add_chunks(text_chunks).await {
                    // Log error but don't fail the operation
                    eprintln!("Warning: Failed to index content for search: {e}");
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to get search engine for indexing: {e}");
            }
        }
    }

    // Track this upload for service context
    {
        let mut recent = server.recent_uploads.lock().await;

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

pub async fn delete_content(
    server: &ContentStoreServer,
    params: Value,
    session_id: &str,
) -> Result<MCPResult, String> {
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

    // Verify the content belongs to the current session
    let content_session_id = {
        let storage = server.storage.lock().await;
        if let Some(sid) = storage.get_content_session_id(&normalized_content_id) {
            sid
        } else {
            return Ok(operation_failed_error(
                "Delete content",
                &format!("Content '{}' not found", args.content_id),
                vec![
                    "Use listContent to see available content".to_string(),
                    "Verify the content ID is correct".to_string(),
                ],
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
    let mut storage = server.storage.lock().await;
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
    match server.get_search_engine(session_id).await {
        Ok(engine_arc) => {
            let mut search_engine = engine_arc.lock().await;
            if let Err(e) = search_engine.remove_chunks(&normalized_content_id).await {
                // Log error but don't fail the operation since content is already deleted
                error!("Failed to remove content from search index: {e}");
            }
        }
        Err(e) => {
            error!("Failed to get search engine for deletion: {e}");
        }
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
