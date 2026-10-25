mod fetch;
mod files;
mod markdown;

pub use fetch::fetch_url;
pub use files::save_raw_html_to_file;

use crate::mcp::builtin::browser::{handle_browser_op_error, BrowserServer};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::services::InteractiveBrowserServer;
use chrono::Utc;
use markdown::convert_to_markdown;
use serde_json::{json, Value};
use tokio::task;

/// Whether a `page` read should extract first because the content cache is empty.
///
/// Agents often call `getPageContent({ "page": 1 })` as the first content read.
/// Routing that straight to the cache produced `Extracted content '<sessionId>' not
/// found` and forced recreate-session loops (Harbor harness cycle 2026-09-20-c18).
pub(crate) fn should_extract_before_cached_page_read(
    has_page_arg: bool,
    has_active_session: bool,
    cache_has_content: bool,
) -> bool {
    has_page_arg && has_active_session && !cache_has_content
}

/// Smart routing for `getPageContent`.
///
/// - No `page`: extract fresh content from the live page.
/// - With `page` and cache present: read that cached page.
/// - With `page` and empty cache: extract first, then return page 1 from the
///   extract response (or read `page` > 1 from the newly filled cache).
pub async fn smart_content(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let has_page_arg = args.get("page").is_some();
    if !has_page_arg {
        return extract_web_content(server, args).await;
    }

    let session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let cache_has_content = session_id
        .as_ref()
        .is_some_and(|id| server.content_store.has_content(id));

    if should_extract_before_cached_page_read(has_page_arg, session_id.is_some(), cache_has_content)
    {
        let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1);
        let mut extract_args = args.clone();
        if let Some(obj) = extract_args.as_object_mut() {
            obj.remove("page");
        }
        let extract_result = extract_web_content(server, extract_args).await?;
        if extract_result.indicates_error() {
            return Ok(extract_result);
        }
        // Extract already returns page-1 content; skip a redundant cache round-trip.
        if page <= 1 {
            return Ok(extract_result);
        }
        // Cache is populated; fall through to read page 2+.
    }

    read_web_content(server, args).await
}

pub async fn extract_web_content(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use browser__createSession FIRST to start a browser session".to_string(),
                "Wait for browser__createSession to return a success message before extracting content"
                    .to_string(),
            ])
            .to_mcp_result());
        }
    };

    let save_raw_html = args
        .get("saveRawHtml")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let auto_merge = args
        .get("autoMerge")
        .and_then(|v| v.as_bool())
        .unwrap_or(true); // Default: true

    // Extract page title and URL
    let page_title = service
        .execute_script(&browser_session_id, "document.title")
        .await
        .unwrap_or_default();

    let current_url = service
        .execute_script(&browser_session_id, "window.location.href")
        .await
        .unwrap_or_default();

    // Extract HTML (body.outerHTML)
    let raw_html = match extract_html_from_page(&service, &browser_session_id).await {
        Ok(html) => html,
        Err(e) => {
            return Ok(handle_browser_op_error(
                "Extract HTML from page",
                e,
                vec![
                    "Verify the browser session is active",
                    "Ensure the page has fully loaded before extracting",
                    "If you need a fresh navigation in this same session, use `browser__navigateToUrl` with a URL",
                    "Try waiting a moment before retrying",
                ],
            ))
        }
    };

    // Convert to markdown
    // CRITICAL: This operation is CPU-intensive and must be offloaded to a blocking thread.
    // Running it on the main async runtime causes "busy loop" behavior and freezes the agent.
    // We also enforce a 10MB limit to prevent OOM crashes and stack overflows in html2md.
    let raw_html_clone = raw_html.clone();
    let markdown_content = task::spawn_blocking(move || {
        // Safety check: Limit input size to 10MB to prevent OOM/crashes
        if raw_html_clone.len() > 10 * 1024 * 1024 {
            return "**Error: Page content too large to process (exceeds 10MB limit).**"
                .to_string();
        }
        convert_to_markdown(&raw_html_clone)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    // Token-based pagination (3000 tokens per page for optimal LLM processing)
    let target_tokens_per_page = 3000;
    let (total_pages, first_page, merged_content, auto_merged, is_unchanged) =
        server.content_store.save_content(
            &browser_session_id,
            markdown_content.clone(),
            target_tokens_per_page,
            auto_merge,
        );

    // Create metadata
    let metadata = create_metadata(
        &markdown_content,
        &raw_html,
        total_pages,
        &page_title,
        &current_url,
    );

    // Build response text
    let mut response_text = if is_unchanged {
        // Return minimal response for unchanged content
        format!(
            "[Content Unchanged]\nPage Title: {}\nURL: {}\n\nThe content of this page has not changed since the last extraction.\nYou can read the previously extracted content using browser__getPageContent({{ \"page\": 1 }}).\n\nIf you need to interact with the page, use browser__listInteractable.",
            if page_title.is_empty() {
                "N/A"
            } else {
                &page_title
            },
            if current_url.is_empty() {
                "N/A"
            } else {
                &current_url
            }
        )
    } else if auto_merged {
        if let Some(content) = &merged_content {
            if total_pages == 1 {
                format!(
                    "[Page 1/1]\n\nPage Title: {}\nURL: {}\n\n{}",
                    if page_title.is_empty() {
                        "N/A"
                    } else {
                        &page_title
                    },
                    if current_url.is_empty() {
                        "N/A"
                    } else {
                        &current_url
                    },
                    content
                )
            } else {
                format!(
                    "✓ Content extracted and auto-merged\n\nPage Title: {}\nURL: {}\n\n{}",
                    if page_title.is_empty() {
                        "N/A"
                    } else {
                        &page_title
                    },
                    if current_url.is_empty() {
                        "N/A"
                    } else {
                        &current_url
                    },
                    content
                )
            }
        } else {
            format!(
                "[Page 1/{}]\n\nPage Title: {}\nURL: {}\n\n{}",
                total_pages,
                if page_title.is_empty() {
                    "N/A"
                } else {
                    &page_title
                },
                if current_url.is_empty() {
                    "N/A"
                } else {
                    &current_url
                },
                first_page
            )
        }
    } else {
        format!(
            "[Page 1/{}]\n\nPage Title: {}\nURL: {}\n\n{}",
            total_pages,
            if page_title.is_empty() {
                "N/A"
            } else {
                &page_title
            },
            if current_url.is_empty() {
                "N/A"
            } else {
                &current_url
            },
            first_page
        )
    };

    // Empty page detection
    if response_text.trim().is_empty() || first_page.trim().is_empty() {
        response_text.push_str(
            "\n\n(Empty Page) The extracted content is empty. This suggests the page might not have loaded correctly or contains no text. Please try calling 'content' again to re-capture the page, or use 'content' with 'saveRawHtml': true to save the raw HTML for inspection."
        );
    }

    // Add pagination footer
    if !auto_merged && total_pages > 1 {
        response_text.push_str(&format!(
            "\n\n--- End of Page 1 ---\nThere are {} pages in total. Use browser__getPageContent({{ \"page\": N }}) to read pages 2-{}.",
            total_pages,
            total_pages
        ));
    } else if !is_unchanged {
        // If not unchanged and (total_pages == 1 or auto_merged), tell user there are no more pages
        response_text.push_str("\n\n(No more pages) All available content has been successfully extracted. There are no additional pages to read.");
    }

    // Save raw HTML if requested
    if save_raw_html {
        match save_raw_html_to_file(&server.agent_session_id, &browser_session_id, &raw_html).await
        {
            Ok(path) => {
                response_text.push_str(&format!(
                    "\n\n--- File Save Information ---\nRaw HTML saved to: {}",
                    path
                ));
            }
            Err(e) => {
                response_text.push_str(&format!("\n\n--- File Save Error ---\n{}", e));
            }
        }
    }

    // Generate unique ID for the response
    let response_id = cuid2::create_id();

    let hint = SuccessHint::new(
        response_text,
        vec![
            "Use browser__listInteractable to see interactive elements".to_string(),
            "Use browser__clickElement to interact with the page".to_string(),
        ],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "id": response_id,
        "content": if auto_merged { merged_content } else { Some(first_page) },
        "format": "markdown",
        "metadata": metadata,
    }))))
}

pub async fn read_web_content(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id.ok_or_else(|| {
        "No active browser session. Call browser__createSession first.".to_string()
    })?;

    let page = match args.get("page").and_then(|v| v.as_u64()) {
        Some(0) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "page must be a positive integer (minimum 1)",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use page: 1 to read the first page".to_string(),
                "page: 0 is not valid (schema minimum is 1)".to_string(),
            ])
            .to_mcp_result())
        }
        Some(p) => p as usize,
        Option::None => return Ok(missing_param_error("page", ToolGroup::Browser)),
    };

    // Check if content exists (smart_content normally extracts on cache miss first).
    if !server.content_store.has_content(&browser_session_id) {
        return Ok(guided_error(
            ErrorCategory::ResourceNotFound,
            "No extracted page content in cache for this browser session",
            ToolGroup::Browser,
        )
        .guidance(vec![
            "Call browser__getPageContent({}) without a page argument to extract fresh content"
                .to_string(),
            "After extraction, use browser__getPageContent({ \"page\": N }) to read additional cached pages"
                .to_string(),
        ])
        .to_mcp_result());
    }

    // Get the requested page
    match server.content_store.get_page(&browser_session_id, page) {
        Some(page_data) => {
            let mut response_text = format!(
                "[Page {}/{}]\n\n{}",
                page_data.page_number, page_data.total_pages, page_data.content
            );

            if page_data.page_number == page_data.total_pages {
                response_text.push_str("\n\n(No more pages) All available content has been successfully extracted. There are no additional pages to read.");
            }

            let hint = SuccessHint::new(
                response_text,
                if page_data.page_number < page_data.total_pages {
                    vec![format!(
                        "Read page {} to continue",
                        page_data.page_number + 1
                    )]
                } else {
                    vec![
                        "All pages read. Content reading is complete".to_string(),
                        "You should now process the gathered information to answer the user's request".to_string(),
                        "If you truly need further actions on this page, use browser__listInteractable".to_string(),
                    ]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "content": page_data.content,
                "page": page_data.page_number,
                "totalPages": page_data.total_pages,
            }))))
        }
        Option::None => {
            let total_pages = server
                .content_store
                .get_total_pages(&browser_session_id)
                .unwrap_or(0);
            let error = ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("Invalid page number: {}. Limit is {}.", page, total_pages),
                vec![
                    format!("There are only {} pages available in total", total_pages),
                    "If you have read the last page, stop requesting more pages".to_string(),
                    "Process the information you have already gathered".to_string(),
                ],
                ToolGroup::Browser,
            );
            Ok(error.to_mcp_result())
        }
    }
}

/// Extract HTML from page (body.outerHTML)
async fn extract_html_from_page(
    service: &InteractiveBrowserServer,
    session_id: &str,
) -> Result<String, String> {
    let script = "document.body ? document.body.outerHTML : \"\"";
    let raw_html = service.execute_script(session_id, script).await?;
    Ok(raw_html)
}

/// Create metadata for extracted content
fn create_metadata(
    content: &str,
    raw_html: &str,
    total_pages: usize,
    page_title: &str,
    current_url: &str,
) -> Value {
    json!({
        "extraction_timestamp": Utc::now().to_rfc3339(),
        "content_length": content.len(),
        "raw_html_size": raw_html.len(),
        "selector": "body",
        "format": "markdown",
        "total_pages": total_pages,
        "pageTitle": page_title,
        "sourceUrl": current_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_read_extracts_when_cache_empty_and_session_active() {
        assert!(should_extract_before_cached_page_read(true, true, false));
        assert!(!should_extract_before_cached_page_read(true, true, true));
        assert!(!should_extract_before_cached_page_read(false, true, false));
        assert!(!should_extract_before_cached_page_read(true, false, false));
    }
}
