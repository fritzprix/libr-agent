use super::search;
use super::server::AttachmentsServer;
use super::types::*;
use crate::mcp::builtin::error_guidance::{
    guided_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use log::error;
use serde_json::Value;

pub async fn list_content(
    server: &AttachmentsServer,
    params: Value,
    session_id: &str,
) -> Result<MCPResult, String> {
    let args: ListContentArgs = if params.is_null() {
        ListContentArgs {
            pagination: Option::None,
        }
    } else {
        match serde_json::from_value(params) {
            Ok(args) => args,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid list parameters: {e}"),
                    ToolGroup::ContentStore,
                )
                .with_guidance(vec!["Check the parameter schema".to_string()])
                .to_mcp_result());
            }
        }
    };

    if let Err(e) = server.ensure_session_store(session_id).await {
        error!(
            "Failed to ensure attachment storage for session {session_id} while listing files: {e}"
        );
        return Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Prepare attachment storage failed for session {session_id}: {e}"),
            ToolGroup::ContentStore,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Ensure session is active".to_string(),
            "Retry the operation".to_string(),
        ])
        .to_mcp_result());
    }

    let (offset, limit) = args.pagination.as_ref().map_or((0usize, 100usize), |p| {
        let offset = p.offset.unwrap_or(0);
        let limit = p.limit.unwrap_or(100).clamp(1, 1000);
        (offset, limit)
    });

    let storage = server.storage.lock().await;
    let (contents, total) = match storage.list_content(session_id, offset, limit).await {
        Ok((contents, total)) => (contents, total),
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("List content failed: {e}"),
                ToolGroup::ContentStore,
            )
            .with_guidance(vec![
                "Check database connectivity".to_string(),
                "Verify session is active".to_string(),
                "Retry the operation".to_string(),
            ])
            .to_mcp_result());
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
            "Found {} of {} attachments{}",
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
            vec!["Use read with a contentId from above to view full content".to_string()]
        } else {
            vec!["Use list to see files in current session".to_string()]
        },
    );

    Ok(hint.to_mcp_result_with_data(Some(serde_json::json!({
        "sessionId": session_id,
        "contents": content_list,
        "total": total,
        "hasMore": has_more
    }))))
}

pub async fn read_content(
    server: &AttachmentsServer,
    params: Value,
    session_id: &str,
) -> Result<MCPResult, String> {
    let args: ReadContentArgs = match serde_json::from_value(params) {
        Ok(args) => args,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Invalid read parameters: {e}"),
                ToolGroup::ContentStore,
            )
            .with_guidance(vec!["Check the parameter schema".to_string()])
            .to_mcp_result());
        }
    };

    // Normalize content ID (add "content_" prefix if missing)
    let normalized_content_id = AttachmentsServer::normalize_content_id(&args.content_id);

    // Verify content belongs to current session
    let content_session_id = {
        let storage = server.storage.lock().await;
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
        return Ok(guided_error(
            ErrorCategory::PermissionDenied,
            format!(
                "Attachment '{}' belongs to a different session",
                args.content_id
            ),
            ToolGroup::ContentStore,
        )
        .with_guidance(vec![
            "Use list to see attachments in current session".to_string(),
            "Switch to the session that owns this attachment".to_string(),
            "Verify the content ID is correct".to_string(),
        ])
        .to_mcp_result());
    }

    // Read content (session verification passed)
    let storage = server.storage.lock().await;

    // Get content metadata for accurate truncation messaging
    let content_item = match storage.get_content_item(&normalized_content_id) {
        Some(item) => item,
        None => {
            return Ok(not_found_error(
                "Content",
                &args.content_id,
                ToolGroup::ContentStore,
            ));
        }
    };
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
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("Read content failed for '{}': {}", args.content_id, e),
                ToolGroup::ContentStore,
            )
            .with_guidance(vec![
                "Verify the content ID is correct".to_string(),
                "Check line range is valid".to_string(),
                "Use list to see available content".to_string(),
            ])
            .to_mcp_result());
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
                    "To read more, use read with fromLine={}",
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
        "Use search to find specific attachments".to_string(),
        format!(
            "Use delete with contentId='{}' to remove this attachment",
            args.content_id
        ),
    ];

    if let Some(hint) = next_step_hint {
        hints.insert(0, hint);
    }

    let hint = SuccessHint::new(
        format!(
            "Attachment '{}' (lines {}-{}):\n\n{}",
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

pub async fn keyword_similarity_search(
    server: &AttachmentsServer,
    params: Value,
    session_id: &str,
) -> Result<MCPResult, String> {
    let args: KeywordSearchArgs = match serde_json::from_value(params) {
        Ok(args) => args,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Invalid search parameters: {e}"),
                ToolGroup::ContentStore,
            )
            .with_guidance(vec!["Check the parameter schema".to_string()])
            .to_mcp_result());
        }
    };

    if let Err(e) = server.ensure_session_store(session_id).await {
        error!("Failed to ensure attachment storage for session {session_id} during search: {e}");
        return Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Prepare attachment storage failed for session {session_id}: {e}"),
            ToolGroup::ContentStore,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Ensure session is active".to_string(),
            "Retry the operation".to_string(),
        ])
        .to_mcp_result());
    }

    let top_n = args
        .options
        .as_ref()
        .and_then(|opts| opts.top_n)
        .unwrap_or(10)
        .clamp(1, 100);

    let ranking_limit = std::cmp::max(top_n, 50);
    let score_threshold = args.options.as_ref().and_then(|opts| opts.threshold);

    let engine_arc = match server.get_search_engine(session_id).await {
        Ok(engine) => engine,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("Failed to initialize search engine: {e}"),
                ToolGroup::ContentStore,
            )
            .with_guidance(vec![
                "Check filesystem permissions".to_string(),
                "Retry the operation".to_string(),
            ])
            .to_mcp_result());
        }
    };

    let search_engine = engine_arc.lock().await;

    let all_results: Vec<search::SearchResult> =
        match search_engine.search_bm25(&args.query, ranking_limit).await {
            Ok(results) => results,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    format!(
                        "Search attachments failed for query '{}': {}",
                        args.query, e
                    ),
                    ToolGroup::ContentStore,
                )
                .with_guidance(vec![
                    "Verify the search query is valid".to_string(),
                    "Check if attachments have been indexed".to_string(),
                    "Use list to see available attachments".to_string(),
                ])
                .to_mcp_result());
            }
        };

    // Filter results by session_id
    let storage = server.storage.lock().await;
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

    // Build detailed results for agent visibility FIRST (using typed structs)
    let results_text = if filtered_results.is_empty() {
        String::new()
    } else {
        let details: Vec<String> = filtered_results
            .iter()
            .enumerate()
            .map(|(idx, result)| {
                // Sanitize match text to keep indentation valid
                let clean_match = result.matched_text.replace('\n', " ");
                let (start, end) = result.line_range;

                format!(
                    "[{}] Content ID: {}\n    Score: {:.2}\n    Lines: {}-{}\n    Match: {}",
                    idx + 1,
                    result.content_id,
                    result.score,
                    start,
                    end,
                    clean_match
                )
            })
            .collect();
        format!("\n\n{}", details.join("\n\n"))
    };

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
                "Use list to see all available attachments".to_string(),
            ]
        } else {
            vec!["Use read with a contentId from above to view full content".to_string()]
        },
    );

    Ok(hint.to_mcp_result_with_data(Some(serde_json::json!({
        "sessionId": session_id,
        "results": search_results
    }))))
}
