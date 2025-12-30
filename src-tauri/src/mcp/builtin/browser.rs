use crate::mcp::builtin::browser_content_store::BrowserContentStore;
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, not_found_error, operation_failed_error,
    ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::mcp::MCPTool;
use crate::services::InteractiveBrowserServer;
use async_trait::async_trait;
use chrono::Utc;
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Manager};

use super::BuiltinMCPServer;

// Global content store for browser extracted content
static BROWSER_CONTENT_STORE: Lazy<BrowserContentStore> = Lazy::new(BrowserContentStore::new);

/// A built-in MCP server that constructs a bridge to the InteractiveBrowserServer service
#[derive(Debug)]
pub struct BrowserServer {
    app_handle: AppHandle,
    agent_session_id: String,
    // We keep track of the browser session ID associated with this agent session
    browser_session_id: Arc<RwLock<Option<String>>>,
}

impl BrowserServer {
    pub fn new(app_handle: AppHandle, agent_session_id: String) -> Self {
        Self {
            app_handle,
            agent_session_id,
            browser_session_id: Arc::new(RwLock::new(None)), // Initialize lazily
        }
    }

    fn handle_browser_op_error(
        operation: &str,
        error: String,
        default_guidance: Vec<&str>,
    ) -> MCPResult {
        let error_lower = error.to_lowercase();
        let is_timeout = error_lower.contains("timeout") || error_lower.contains("timed out");

        let guidance_strs = if is_timeout {
            vec![
                "The page load timed out. This often happens with complex sites.",
                "Try creating a new session with 'createSession' to reset the state.",
                "If the problem persists, the site might be blocking automated access.",
            ]
        } else {
            default_guidance
        };

        let guidance: Vec<String> = guidance_strs.iter().map(|s| s.to_string()).collect();

        operation_failed_error(operation, &error, guidance, ToolGroup::Browser)
    }

    /// Get the browser service from Tauri state
    fn get_browser_service(&self) -> Result<InteractiveBrowserServer, String> {
        self.app_handle
            .try_state::<InteractiveBrowserServer>()
            .map(|s| s.inner().clone())
            .ok_or_else(|| "InteractiveBrowserServer state not found".to_string())
    }

    /// Helper to inline the clickElement script
    fn get_click_script(selector: &str) -> String {
        format!(
            r#"(function() {{
                const el = document.querySelector({});
                if (el) {{
                    el.scrollIntoView({{block: 'center'}});
                    el.focus();
                    el.click();
                    return 'Clicked element';
                }}
                return 'Element not found';
            }})()"#,
            serde_json::to_string(selector).unwrap()
        )
    }

    /// Helper to inline the listInteractable filter script
    fn get_filter_script(filter_type: &str, scope: &str) -> String {
        // Simplified version of the filter script for Rust embedding
        // Note: Full version from ListInteractableTool.ts is quite long.
        // We will implement a functional subset that covers the core logic.
        let filter_selector = match filter_type {
            "semantic_input" => "input:not([type=\"hidden\"]):not([disabled]), select:not([disabled]), textarea:not([disabled]), [contenteditable=\"true\"]",
            "all_focusable" => "a, button, input, select, textarea, [tabindex]:not([tabindex=\"-1\"]), [contenteditable]",
            _ => "a[href], button:not([disabled]), [role=\"button\"]:not([disabled]), [onclick], [role=\"link\"]" // default semantic_clickable
        };

        let scope_check = if scope == "viewport" {
            r#"
            const rect = el.getBoundingClientRect();
            if (rect.width === 0 || rect.height === 0) return false;
            const inViewport = (
                rect.top < window.innerHeight &&
                rect.bottom > 0 &&
                rect.left < window.innerWidth &&
                rect.right > 0
            );
            if (!inViewport) return false;
            "#
        } else {
            r#"
             const rect = el.getBoundingClientRect();
             if (rect.width === 0 || rect.height === 0) return false;
             "#
        };

        format!(
            r#"(function() {{
                const selector = "{}";
                const candidates = Array.from(document.querySelectorAll(selector));
                
                function isVisible(el) {{
                    const style = window.getComputedStyle(el);
                    if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') return false;
                    {}
                    return true;
                }}

                function getUniqueSelector(el) {{
                    if (el.id) return '#' + CSS.escape(el.id);
                    // Minimal fallback
                    return el.tagName.toLowerCase(); 
                }}

                const visible = candidates.filter(isVisible).slice(0, 50).map((el, idx) => {{
                    return {{
                        index: idx,
                        tag: el.tagName.toLowerCase(),
                        text: (el.textContent || '').trim().substring(0, 50),
                        attributes: {{
                            href: el.getAttribute('href'),
                            type: el.getAttribute('type'),
                            placeholder: el.getAttribute('placeholder'),
                            "aria-label": el.getAttribute('aria-label')
                        }},
                        selector: getUniqueSelector(el)
                    }};
                }});

                return JSON.stringify(visible);
            }})()"#,
            filter_selector.replace("\"", "\\\""), // simple escape
            scope_check
        )
    }

    /// Format interactive elements list to match TypeScript output format
    /// Matches formatSmartResults() from ListInteractableTool.ts
    fn format_interactive_elements(
        json_result: &str,
        filter_type: &str,
        scope: &str,
    ) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Element {
            index: usize,
            tag: String,
            text: String,
            attributes: serde_json::Map<String, Value>,
            selector: String,
        }

        let elements: Vec<Element> = serde_json::from_str(json_result)
            .map_err(|e| format!("Failed to parse elements JSON: {}", e))?;

        if elements.is_empty() {
            let filter_label = filter_type.replace('_', " ");
            let scope_label = if scope == "viewport" {
                "current viewport"
            } else {
                "page"
            };
            return Ok(format!(
                "No {} elements found in {}.",
                filter_label, scope_label
            ));
        }

        // Header with metadata
        let filter_label = filter_type.replace('_', " ");
        let scope_label = if scope == "viewport" {
            "viewport"
        } else {
            "page"
        };
        let mut output = format!(
            "Found {} {} element(s) in {}:\n\n",
            elements.len(),
            filter_label,
            scope_label
        );

        // Format each element
        for el in &elements {
            // Format attributes
            let attrs: Vec<String> = el
                .attributes
                .iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| {
                    if let Some(s) = v.as_str() {
                        format!("{}=\"{}\"", k, s)
                    } else {
                        String::new()
                    }
                })
                .filter(|s| !s.is_empty())
                .collect();

            let attr_str = if !attrs.is_empty() {
                format!(" {}", attrs.join(" "))
            } else {
                String::new()
            };

            let text_str = if !el.text.is_empty() {
                format!(" \"{}\"", el.text)
            } else {
                String::new()
            };

            output.push_str(&format!(
                "[{}] <{}{}>{}\n",
                el.index, el.tag, attr_str, text_str
            ));
            output.push_str(&format!("    Selector: {}\n\n", el.selector));
        }

        // Footer with usage hint
        output.push_str("💡 Use the selector or index to interact with these elements.");

        Ok(output)
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
        // We use a "less destructive" approach by transforming table tags to generic divs/spans
        // This preserves the DOM structure for the parser while preventing ASCII table generation.

        // 1. Remove script, style, noscript (using non-greedy dot matches with 's' flag for newlines)
        // Note: We use separate regexes to avoid backreference issues
        let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
        let s = re_script.replace_all(raw_html, "");

        let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
        let s = re_style.replace_all(&s, "");

        let re_noscript = regex::Regex::new(r"(?is)<noscript[^>]*>.*?</noscript>").unwrap();
        let s = re_noscript.replace_all(&s, "");

        // 2. Flatten tables by transmuting them to divs/spans
        // This keeps the tree balanced (preventing parser crashes) but removes table semantics

        // <table...> -> <div>
        let re_table = regex::Regex::new(r"(?i)<table[^>]*>").unwrap();
        let s = re_table.replace_all(&s, "<div>");
        let s = s.replace("</table>", "</div>");

        // <tr...> -> <div> (Rows become blocks, ensuring newlines)
        let re_tr = regex::Regex::new(r"(?i)<tr[^>]*>").unwrap();
        let s = re_tr.replace_all(&s, "<div>");
        let s = s.replace("</tr>", "</div>");

        // <td...> -> <span> (Cells become inline text)
        // We add a space to ensure separation between cell contents
        let re_td = regex::Regex::new(r"(?i)<(td|th)[^>]*>").unwrap();
        let s = re_td.replace_all(&s, "<span> ");
        let re_td_close = regex::Regex::new(r"(?i)</(td|th)>").unwrap();
        let s = re_td_close.replace_all(&s, "</span>");

        // Remove other table structural tags (thead, tbody, tfoot)
        let re_tbody = regex::Regex::new(r"(?i)</?(thead|tbody|tfoot)[^>]*>").unwrap();
        let s = re_tbody.replace_all(&s, "");

        // 3. Convert to Markdown
        let markdown = html2md::parse_html(&s);

        // Apply legacy cleaning rules to match ExtractContentTool.ts behavior exactly
        // 1. Replace 2+ newlines with 1 (collapses paragraphs)
        let re_newlines = regex::Regex::new(r"\n{2,}").unwrap();
        let s = re_newlines.replace_all(&markdown, "\n");

        // 2. Remove trailing spaces before newline
        let re_trailing = regex::Regex::new(r"[ \t]+\n").unwrap();
        let s = re_trailing.replace_all(&s, "\n");

        // 3. Remove leading spaces after newline (Note: this flattens indentation)
        let re_leading = regex::Regex::new(r"\n[ \t]+").unwrap();
        let s = re_leading.replace_all(&s, "\n");

        // 4. Replace multiple spaces/tabs with single space
        let re_spaces = regex::Regex::new(r"[ \t]{2,}").unwrap();
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
    async fn save_raw_html(
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
}

#[async_trait]
impl BuiltinMCPServer for BrowserServer {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Capabilities to control a web browser, navigate to URLs, and extract content."
    }

    async fn get_service_context(
        &self,
        _options: Option<&Value>,
    ) -> crate::mcp::types::ServiceContext {
        let session_id_opt = {
            self.browser_session_id
                .read()
                .ok()
                .and_then(|id| id.clone())
        };

        if let Some(session_id) = session_id_opt {
            // Try to get service and fetch state
            if let Ok(service) = self.get_browser_service() {
                // Fetch URL
                let url = match service
                    .execute_script(&session_id, "window.location.href")
                    .await
                {
                    Ok(u) => u.trim_matches('"').to_string(), // Js returns string with quotes often
                    Err(_) => "Unknown".to_string(),
                };

                // Fetch Title
                let title = match service.execute_script(&session_id, "document.title").await {
                    Ok(t) => t.trim_matches('"').to_string(),
                    Err(_) => "Unknown".to_string(),
                };

                return crate::mcp::types::ServiceContext {
                    context_prompt: format!(
                        "## Browser\n\
                        **Active Session**: {}\n\
                        **Current URL**: {}\n\
                        **Page Title**: {}",
                        session_id, url, title
                    ),
                    structured_state: None,
                };
            }
        }

        // Fallback or when no session is active
        crate::mcp::types::ServiceContext {
            context_prompt: "## Browser\n\
                **Status**: No active session\n\
                *Use `createSession` to start*"
                .to_string(),
            structured_state: None,
        }
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "createSession".to_string(),
                description:
                    "Create a new browser session. Must be called before other browser tools."
                        .to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "Initial URL to open (default: about:blank)"
                        },
                        "title": {
                            "type": "string",
                            "description": "Optional title for the session window"
                        }
                    },
                    "required": []
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "navigateToUrl".to_string(),
                description: "Navigate to a specific URL in the browser.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        },
                        "url": {
                            "type": "string",
                            "description": "The URL to navigate to (e.g., https://google.com)"
                        }
                    },
                    "required": ["sessionId", "url"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "navigateBack".to_string(),
                description: "Navigate back in the browser history.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                         "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        }
                    },
                    "required": ["sessionId"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "navigateForward".to_string(),
                description: "Navigate forward in the browser history.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                         "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        }
                    },
                    "required": ["sessionId"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "extractWebContent".to_string(),
                description: "Convert the webpage into clean, readable markdown format. Automatically merges content if small (≤2 pages OR <5000 chars), otherwise returns first page with total page count. Use readWebContent for subsequent pages.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                         "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        },
                        "saveRawHtml": {
                            "type": "boolean",
                            "description": "Save raw HTML for DOM analysis. Default: false"
                        }
                    },
                    "required": ["sessionId"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "getCurrentUrl".to_string(),
                description: "Gets the current URL of the browser page.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                         "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        }
                    },
                    "required": ["sessionId"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "getPageTitle".to_string(),
                description: "Gets the title of the current browser page.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                         "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        }
                    },
                    "required": ["sessionId"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "clickElement".to_string(),
                description: "Clicks on a DOM element using CSS selector.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        },
                        "selector": {
                            "type": "string",
                            "description": "CSS selector of the element to click"
                        }
                    },
                    "required": ["sessionId", "selector"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "inputText".to_string(),
                description: "Inputs text into a form field using CSS selector.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        },
                        "selector": {
                            "type": "string",
                            "description": "CSS selector of the input field"
                        },
                        "text": {
                            "type": "string",
                            "description": "Text to input"
                        }
                    },
                    "required": ["sessionId", "selector", "text"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "scrollPage".to_string(),
                description: "Scrolls the page to specified coordinates.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        },
                        "x": {
                            "type": "number",
                            "description": "X coordinate to scroll to"
                        },
                        "y": {
                            "type": "number",
                            "description": "Y coordinate to scroll to"
                        }
                    },
                    "required": ["sessionId", "x", "y"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "listInteractable".to_string(),
                description: "Lists interactable elements on the page.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "sessionId": {
                            "type": "string",
                            "description": "The browser session ID"
                        },
                        "filterType": {
                            "type": "string",
                            "enum": ["semantic_clickable", "semantic_input", "all_focusable"],
                            "description": "Filter type (default: semantic_clickable)"
                        },
                        "scope": {
                            "type": "string",
                            "enum": ["viewport", "all"],
                            "description": "Scope of listing (default: viewport)"
                        }
                    },
                    "required": ["sessionId"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "closeSession".to_string(),
                description: "Close the browser session.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                         "sessionId": {
                            "type": "string",
                            "description": "The ID of the session to use"
                        }
                    },
                    "required": ["sessionId"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "readWebContent".to_string(),
                description: "Read a specific page from previously extracted web content.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "sessionId": {
                            "type": "string",
                            "description": "The ID of the browser session"
                        },
                        "page": {
                            "type": "number",
                            "description": "Page number to read (1-based index)"
                        }
                    },
                    "required": ["sessionId", "page"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        let service = self.get_browser_service()?;

        // For closeSession, we don't need to create a session if one doesn't exist
        if tool_name == "closeSession" {
            let id_opt = {
                let lock = self.browser_session_id.read().map_err(|e| e.to_string())?;
                lock.clone()
            };

            if let Some(id) = id_opt {
                match service.close_session(&id).await {
                    Ok(_) => {}
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Close browser session",
                            &e,
                            vec![
                                "Verify the browser session is still active".to_string(),
                                "Use createSession to start a new session if needed".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                }
                {
                    let mut lock = self.browser_session_id.write().map_err(|e| e.to_string())?;
                    *lock = None;
                }

                let hint = SuccessHint::new(
                    "Browser session closed",
                    vec!["Use createSession to start a new browser session".to_string()],
                );
                return Ok(hint.to_mcp_result());
            } else {
                let warning = ErrorGuidance::new(
                    ErrorCategory::InvalidState,
                    "No active browser session to close",
                    ToolGroup::Browser,
                );
                return Ok(warning.to_mcp_result());
            }
        }

        // For createSession, explicitly create a new session
        if tool_name == "createSession" {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("about:blank");

            // Check if session already exists
            {
                let id_lock = self.browser_session_id.read().map_err(|e| e.to_string())?;
                if let Some(id) = id_lock.as_ref() {
                    let warning = ErrorGuidance::new(
                        ErrorCategory::DuplicateResource,
                        format!("Session already exists: {}", id),
                        ToolGroup::Browser,
                    );
                    return Ok(warning.to_mcp_result());
                }
            }

            let (id, status_msg) = match service
                .create_browser_session(url, Some(&format!("Agent {}", self.agent_session_id)))
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    return Ok(operation_failed_error(
                        "Create browser session",
                        &e,
                        vec![
                            "Verify the URL format is valid (must include http:// or https://)"
                                .to_string(),
                            "Check browser service is available and running".to_string(),
                        ],
                        ToolGroup::Browser,
                    ))
                }
            };

            {
                let mut id_lock = self.browser_session_id.write().map_err(|e| e.to_string())?;
                *id_lock = Some(id.clone());
            }

            let hint = SuccessHint::new(
                format!("Browser session created: {}. {}", id, status_msg),
                vec![
                    "Use navigateToUrl to load a webpage".to_string(),
                    "Use extractWebContent to see the initial page".to_string(),
                ],
            );
            return Ok(hint.to_mcp_result());
        }

        match tool_name {
            "navigateToUrl" => {
                let url = match args.get("url").and_then(|v| v.as_str()) {
                    Some(u) => u,
                    None => return Ok(missing_param_error("url", ToolGroup::Browser)),
                };
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };

                // Proactive URL validation
                if !url.starts_with("http://")
                    && !url.starts_with("https://")
                    && !url.starts_with("about:")
                {
                    return Ok(invalid_input_error(
                        &format!(
                            "Invalid URL format: '{}'. Must start with http://, https://, or about:",
                            url
                        ),
                        ToolGroup::Browser,
                    ));
                }

                let result = match service.navigate_to_url(session_id, url).await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(Self::handle_browser_op_error(
                            "Navigate to URL",
                            e,
                            vec![
                                "Verify the URL format is valid (must include http:// or https://)",
                                "Use createSession to start a new browser session",
                                "Check if the session still exists",
                                "Try checking if the page loaded using getPageTitle",
                            ],
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec![
                        "Use extractWebContent to see page content".to_string(),
                        "Use listInteractable to see clickable elements".to_string(),
                    ],
                );
                Ok(hint.to_mcp_result())
            }
            "navigateBack" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };

                let result = match service.navigate_back(session_id).await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(Self::handle_browser_op_error(
                            "Navigate back",
                            e,
                            vec![
                                "Ensure there is a previous page in history",
                                "Use getCurrentUrl to check current page",
                            ],
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec!["Use extractWebContent to see page content after navigation".to_string()],
                );
                Ok(hint.to_mcp_result())
            }
            "navigateForward" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };

                let result = match service.navigate_forward(session_id).await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(Self::handle_browser_op_error(
                            "Navigate forward",
                            e,
                            vec![
                                "Ensure there is a next page in history",
                                "Use getCurrentUrl to check current page",
                            ],
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec!["Use extractWebContent to see page content after navigation".to_string()],
                );
                Ok(hint.to_mcp_result())
            }
            "getCurrentUrl" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };

                let result = match service
                    .execute_script(session_id, "window.location.href")
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Get current URL",
                            &e,
                            vec![
                                "Verify the browser session is active".to_string(),
                                "Use createSession to start a new session if needed".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec!["Use navigateToUrl to navigate to a different URL".to_string()],
                );
                Ok(hint.to_mcp_result())
            }
            "getPageTitle" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };

                let result = match service.execute_script(session_id, "document.title").await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Get page title",
                            &e,
                            vec![
                                "Verify the browser session is active".to_string(),
                                "Ensure the page has fully loaded".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec!["Use extractWebContent to see full page content".to_string()],
                );
                Ok(hint.to_mcp_result())
            }
            "extractWebContent" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };

                let save_raw_html = args
                    .get("saveRawHtml")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let auto_merge = true; // Always true by default

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
                let raw_html = match Self::extract_html_from_page(&service, session_id).await {
                    Ok(html) => html,
                    Err(e) => {
                        return Ok(Self::handle_browser_op_error(
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
                let markdown_content = Self::convert_to_markdown(&raw_html);

                // Pagination
                let (total_pages, first_page, merged_content, auto_merged) = BROWSER_CONTENT_STORE
                    .save_content(session_id, markdown_content.clone(), 6000, auto_merge);

                // Create metadata
                let metadata = Self::create_metadata(
                    &markdown_content,
                    &raw_html,
                    total_pages,
                    &page_title,
                    &current_url,
                );

                // Build response text
                let mut response_text = if auto_merged {
                    if let Some(content) = &merged_content {
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
                        "\n\n--- End of Page 1 ---\nThere are {} pages in total. Use readWebContent(sessionId, page) to read more, or use autoMerge: true to get all content at once.",
                        total_pages
                    ));
                }

                // Save raw HTML if requested
                if save_raw_html {
                    match Self::save_raw_html(&self.app_handle, session_id, &raw_html).await {
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
            "clickElement" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };
                let selector = match args.get("selector").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return Ok(missing_param_error("selector", ToolGroup::Browser)),
                };

                // Proactive CSS selector validation
                if selector.trim().is_empty() {
                    return Ok(invalid_input_error(
                        "CSS selector cannot be empty",
                        ToolGroup::Browser,
                    ));
                }

                let script = Self::get_click_script(selector);
                let result = match service.execute_script(session_id, &script).await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Click element",
                            &e,
                            vec![
                                "Verify the selector is correct CSS syntax".to_string(),
                                "Use listInteractable to find valid selectors".to_string(),
                                "Ensure the element is visible and interactable".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec![
                        "Use extractWebContent to see page changes after click".to_string(),
                        "Use getCurrentUrl to check if navigation occurred".to_string(),
                    ],
                );
                Ok(hint.to_mcp_result())
            }
            "inputText" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };
                let selector = match args.get("selector").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return Ok(missing_param_error("selector", ToolGroup::Browser)),
                };
                let text = match args.get("text").and_then(|v| v.as_str()) {
                    Some(t) => t,
                    None => return Ok(missing_param_error("text", ToolGroup::Browser)),
                };

                // Proactive CSS selector validation
                if selector.trim().is_empty() {
                    return Ok(invalid_input_error(
                        "CSS selector cannot be empty",
                        ToolGroup::Browser,
                    ));
                }

                let script = format!(
                    r#"(function() {{
                        const el = document.querySelector({});
                        if (el) {{
                            el.value = {};
                            el.dispatchEvent(new Event('input', {{bubbles: true}}));
                            el.dispatchEvent(new Event('change', {{bubbles: true}}));
                            return 'Input successful';
                        }}
                        return 'Element not found';
                    }})()"#,
                    serde_json::to_string(selector).unwrap(),
                    serde_json::to_string(text).unwrap()
                );

                let result = match service.execute_script(session_id, &script).await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Input text",
                            &e,
                            vec![
                                "Verify the selector targets an input/textarea element".to_string(),
                                "Use listInteractable with filterType='semantic_input'".to_string(),
                                "Check the element is not disabled or readonly".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec![
                        "Use clickElement to submit the form or click buttons".to_string(),
                        "Use extractWebContent to verify input changes".to_string(),
                    ],
                );
                Ok(hint.to_mcp_result())
            }
            "scrollPage" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };
                let x = match args.get("x").and_then(|v| v.as_f64()) {
                    Some(x_val) => x_val,
                    None => return Ok(missing_param_error("x", ToolGroup::Browser)),
                };
                let y = match args.get("y").and_then(|v| v.as_f64()) {
                    Some(y_val) => y_val,
                    None => return Ok(missing_param_error("y", ToolGroup::Browser)),
                };

                let script = format!("window.scrollTo({}, {}); 'Scrolled'", x, y);
                let result = match service.execute_script(session_id, &script).await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Scroll page",
                            &e,
                            vec![
                                "Verify the browser session is active".to_string(),
                                "Check if the page has scrollable content".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec![
                        "Use listInteractable to see newly visible elements".to_string(),
                        "Use extractWebContent to see content after scroll".to_string(),
                    ],
                );
                Ok(hint.to_mcp_result())
            }
            "listInteractable" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };
                let filter_type = args
                    .get("filterType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("semantic_clickable");
                let scope = args
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or("viewport");

                // Proactive filterType validation
                let valid_filters = ["semantic_clickable", "semantic_input", "all_focusable"];
                if !valid_filters.contains(&filter_type) {
                    return Ok(invalid_input_error(
                        &format!(
                            "Invalid filterType: '{}'. Must be one of: {}",
                            filter_type,
                            valid_filters.join(", ")
                        ),
                        ToolGroup::Browser,
                    ));
                }

                let script = Self::get_filter_script(filter_type, scope);
                let result_json = match service.execute_script(session_id, &script).await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "List interactable elements",
                            &e,
                            vec![
                                "Verify the browser session is active".to_string(),
                                "Ensure the page has fully loaded".to_string(),
                                "Try extractWebContent first to see page structure".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                };

                // Parse and format results to match TypeScript version
                let formatted_text = match BrowserServer::format_interactive_elements(
                    &result_json,
                    filter_type,
                    scope,
                ) {
                    Ok(text) => text,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Format interactable elements",
                            &e,
                            vec![
                                "The page may have returned unexpected data".to_string(),
                                "Try refreshing the page with navigateToUrl".to_string(),
                                "Use extractWebContent to verify page structure".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    formatted_text,
                    vec![
                        "Use clickElement with the selector or index".to_string(),
                        "Use extractWebContent to see full page content".to_string(),
                    ],
                );
                Ok(hint.to_mcp_result())
            }
            "inject_javascript" => {
                let script = match args.get("script").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return Ok(missing_param_error("script", ToolGroup::Browser)),
                };
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };

                let result = match service.execute_script(session_id, script).await {
                    Ok(res) => res,
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Execute JavaScript",
                            &e,
                            vec![
                                "Verify the JavaScript syntax is correct".to_string(),
                                "Check the browser session is active".to_string(),
                                "Ensure the script returns a serializable value".to_string(),
                            ],
                            ToolGroup::Browser,
                        ))
                    }
                };

                let hint = SuccessHint::new(
                    result,
                    vec![
                        "Use extractWebContent to see page changes after script execution"
                            .to_string(),
                    ],
                );
                Ok(hint.to_mcp_result())
            }
            "readWebContent" => {
                let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
                };

                let page = match args.get("page").and_then(|v| v.as_u64()) {
                    Some(p) => p as usize,
                    None => return Ok(missing_param_error("page", ToolGroup::Browser)),
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
                                vec!["All pages read. Use extractWebContent to refresh content"
                                    .to_string()]
                            },
                        );

                        Ok(hint.to_mcp_result_with_data(Some(json!({
                            "content": page_data.content,
                            "page": page_data.page_number,
                            "totalPages": page_data.total_pages,
                        }))))
                    }
                    None => {
                        let error = ErrorGuidance::with_guidance(
                            ErrorCategory::InvalidInput,
                            format!("Invalid page number: {}", page),
                            vec![
                                "Use extractWebContent to see available pages".to_string(),
                                format!("Page number must be between 1 and total pages"),
                            ],
                            ToolGroup::Browser,
                        );
                        Ok(error.to_mcp_result())
                    }
                }
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
