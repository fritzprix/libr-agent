use crate::mcp::builtin::browser::{handle_browser_op_error, BrowserServer};
use crate::mcp::builtin::browser_content_store::BrowserContentStore;
use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::services::InteractiveBrowserServer;
use chrono::Utc;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::task;

// Global content store for browser extracted content (module-scoped)
static BROWSER_CONTENT_STORE: Lazy<BrowserContentStore> = Lazy::new(BrowserContentStore::new);

pub async fn extract_web_content(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;
    let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
        Some(id) => id,
        Option::None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
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
        .execute_script(session_id, "document.title")
        .await
        .unwrap_or_default();

    let current_url = service
        .execute_script(session_id, "window.location.href")
        .await
        .unwrap_or_default();

    // Extract HTML (body.outerHTML)
    let raw_html = match extract_html_from_page(&service, session_id).await {
        Ok(html) => html,
        Err(e) => {
            return Ok(handle_browser_op_error(
                "Extract HTML from page",
                e,
                vec![
                    "Verify the browser session is active",
                    "Ensure the page has fully loaded before extracting",
                    "Use navigateToUrl to reload the page",
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
    let (total_pages, first_page, merged_content, auto_merged) = BROWSER_CONTENT_STORE
        .save_content(
            session_id,
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
    let mut response_text = if auto_merged {
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
            "\n\n(Empty Page) The extracted content is empty. This suggests the page might not have loaded correctly or contains no text. Please try calling 'extractWebContent' again to re-capture the page, or use 'extractWebContent' with 'saveRawHtml': true to save the raw HTML for inspection."
        );
    }

    // Add pagination footer
    if !auto_merged && total_pages > 1 {
        response_text.push_str(&format!(
            "\n\n--- End of Page 1 ---\nThere are {} pages in total. Use readWebContent(sessionId, page) to read pages 2-{}.",
            total_pages,
            total_pages
        ));
    }

    // Save raw HTML if requested
    if save_raw_html {
        match save_raw_html_to_file(&server.app_handle, session_id, &raw_html).await {
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
            "Use listInteractable to see interactive elements".to_string(),
            "Use clickElement to interact with the page".to_string(),
        ],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "id": response_id,
        "content": if auto_merged { merged_content } else { Some(first_page) },
        "format": "markdown",
        "metadata": metadata,
    }))))
}

pub async fn read_web_content(_server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
        Some(id) => id,
        Option::None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
    };

    let page = match args.get("page").and_then(|v| v.as_u64()) {
        Some(p) => p as usize,
        Option::None => return Ok(missing_param_error("page", ToolGroup::Browser)),
    };

    // Check if content exists
    if !BROWSER_CONTENT_STORE.has_content(session_id) {
        return Ok(not_found_error(
            "Extracted content",
            session_id,
            ToolGroup::Browser,
        ));
    }

    // Get the requested page
    match BROWSER_CONTENT_STORE.get_page(session_id, page) {
        Some(page_data) => {
            let response_text = format!(
                "[Page {}/{}]\n\n{}",
                page_data.page_number, page_data.total_pages, page_data.content
            );

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
                        "If you truly need further actions on this page, use listInteractable".to_string(),
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
            let total_pages = BROWSER_CONTENT_STORE
                .get_total_pages(session_id)
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

/// Convert HTML to clean markdown
fn convert_to_markdown(raw_html: &str) -> String {
    // Pre-process HTML to match legacy behavior (remove scripts, flatten tables)

    // 1. Remove script, style, noscript
    let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let s = re_script.replace_all(raw_html, "");

    let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let s = re_style.replace_all(&s, "");

    let re_noscript = Regex::new(r"(?is)<noscript[^>]*>.*?</noscript>").unwrap();
    let s = re_noscript.replace_all(&s, "");

    // 2. Flatten tables by transmuting them to divs/spans
    // <table...> -> <div>
    let re_table = Regex::new(r"(?i)<table[^>]*>").unwrap();
    let s = re_table.replace_all(&s, "<div>");
    let s = s.replace("</table>", "</div>");

    // <tr...> -> <div>
    let re_tr = Regex::new(r"(?i)<tr[^>]*>").unwrap();
    let s = re_tr.replace_all(&s, "<div>");
    let s = s.replace("</tr>", "</div>");

    // <td...> -> <span>
    let re_td = Regex::new(r"(?i)<(td|th)[^>]*>").unwrap();
    let s = re_td.replace_all(&s, "<span> ");
    let re_td_close = Regex::new(r"(?i)</(td|th)>").unwrap();
    let s = re_td_close.replace_all(&s, "</span>");

    // Remove other table structural tags
    let re_tbody = Regex::new(r"(?i)</?(thead|tbody|tfoot)[^>]*>").unwrap();
    let s = re_tbody.replace_all(&s, "");

    // 3. Convert to Markdown
    let markdown = html2md::parse_html(&s);

    // Apply legacy cleaning rules
    // 1. Replace 2+ newlines with 1 (collapses paragraphs)
    let re_newlines = Regex::new(r"\n{2,}").unwrap();
    let s = re_newlines.replace_all(&markdown, "\n");

    // 2. Remove trailing spaces before newline
    let re_trailing = Regex::new(r"[ \t]+\n").unwrap();
    let s = re_trailing.replace_all(&s, "\n");

    // 3. Remove leading spaces after newline
    let re_leading = Regex::new(r"\n[ \t]+").unwrap();
    let s = re_leading.replace_all(&s, "\n");

    // 4. Replace multiple spaces/tabs with single space
    let re_spaces = Regex::new(r"[ \t]{2,}").unwrap();
    let s = re_spaces.replace_all(&s, " ");

    s.trim().to_string()
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

/// Save raw HTML to file
async fn save_raw_html_to_file(
    app_handle: &AppHandle,
    session_id: &str,
    raw_html: &str,
) -> Result<String, String> {
    use crate::services::secure_file_manager::SecureFileManager;

    let file_manager = app_handle
        .try_state::<SecureFileManager>()
        .ok_or("SecureFileManager not found")?;

    let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let file_name = format!("extracted-{}-{}.html", session_id, timestamp);
    let relative_path = format!("extracted-content/{}", file_name);

    file_manager
        .write_file_string(&relative_path, raw_html)
        .await?;

    Ok(relative_path)
}
