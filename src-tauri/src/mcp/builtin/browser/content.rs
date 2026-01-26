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

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

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
    let (total_pages, first_page, merged_content, auto_merged, is_unchanged) =
        BROWSER_CONTENT_STORE.save_content(
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
            "[Content Unchanged]\nPage Title: {}\nURL: {}\n\nThe content of this page has not changed since the last extraction.\nYou can read the previously extracted content using readWebContent(sessionId, page: 1).\n\nIf you need to interact with the page, use listInteractable.",
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
    } else if !is_unchanged {
        // If not unchanged and (total_pages == 1 or auto_merged), tell user there are no more pages
        response_text.push_str("\n\n(No more pages) All available content has been successfully extracted. There are no additional pages to read.");
    }

    // Save raw HTML if requested
    if save_raw_html {
        match save_raw_html_to_file(&server.app_handle, &browser_session_id, &raw_html).await {
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

pub async fn read_web_content(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let page = match args.get("page").and_then(|v| v.as_u64()) {
        Some(p) => p as usize,
        Option::None => return Ok(missing_param_error("page", ToolGroup::Browser)),
    };

    // Check if content exists
    if !BROWSER_CONTENT_STORE.has_content(&browser_session_id) {
        return Ok(not_found_error(
            "Extracted content",
            &browser_session_id,
            ToolGroup::Browser,
        ));
    }

    // Get the requested page
    match BROWSER_CONTENT_STORE.get_page(&browser_session_id, page) {
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

// Compiled regexes for HTML cleaning
static RE_SCRIPT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<script[^>]*>.*?</script>").expect("Invalid regex: RE_SCRIPT"));
static RE_STYLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").expect("Invalid regex: RE_STYLE"));
static RE_NOSCRIPT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?is)<noscript[^>]*>.*?</noscript>").expect("Invalid regex: RE_NOSCRIPT")
});
static RE_TABLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)<table[^>]*>").expect("Invalid regex: RE_TABLE"));
static RE_TR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)<tr[^>]*>").expect("Invalid regex: RE_TR"));
static RE_TD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)<(td|th)[^>]*>").expect("Invalid regex: RE_TD"));
static RE_TD_CLOSE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)</(td|th)>").expect("Invalid regex: RE_TD_CLOSE"));
static RE_TBODY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)</?(thead|tbody|tfoot)[^>]*>").expect("Invalid regex: RE_TBODY"));
static RE_NEWLINES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\n{2,}").expect("Invalid regex: RE_NEWLINES"));
static RE_TRAILING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[ \t]+\n").expect("Invalid regex: RE_TRAILING"));
static RE_LEADING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\n[ \t]+").expect("Invalid regex: RE_LEADING"));
static RE_SPACES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[ \t]{2,}").expect("Invalid regex: RE_SPACES"));

/// Convert HTML to clean markdown
fn convert_to_markdown(raw_html: &str) -> String {
    // Pre-process HTML to match legacy behavior (remove scripts, flatten tables)

    // 1. Remove script, style, noscript
    let s = RE_SCRIPT.replace_all(raw_html, "");
    let s = RE_STYLE.replace_all(&s, "");
    let s = RE_NOSCRIPT.replace_all(&s, "");

    // 2. Flatten tables by transmuting them to divs/spans
    // <table...> -> <div>
    let s = RE_TABLE.replace_all(&s, "<div>");
    let s = s.replace("</table>", "</div>");

    // <tr...> -> <div>
    let s = RE_TR.replace_all(&s, "<div>");
    let s = s.replace("</tr>", "</div>");

    // <td...> -> <span>
    let s = RE_TD.replace_all(&s, "<span> ");
    let s = RE_TD_CLOSE.replace_all(&s, "</span>");

    // Remove other table structural tags
    let s = RE_TBODY.replace_all(&s, "");

    // 3. Convert to Markdown
    let markdown = html2md::parse_html(&s);

    // Apply legacy cleaning rules
    // 1. Replace 2+ newlines with 1 (collapses paragraphs)
    let s = RE_NEWLINES.replace_all(&markdown, "\n");

    // 2. Remove trailing spaces before newline
    let s = RE_TRAILING.replace_all(&s, "\n");

    // 3. Remove leading spaces after newline
    let s = RE_LEADING.replace_all(&s, "\n");

    // 4. Replace multiple spaces/tabs with single space
    let s = RE_SPACES.replace_all(&s, " ");

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
