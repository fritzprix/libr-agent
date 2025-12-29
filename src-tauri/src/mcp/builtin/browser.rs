use crate::mcp::types::MCPResult;
use crate::mcp::MCPTool;
use crate::services::InteractiveBrowserServer;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Manager};

use super::BuiltinMCPServer;

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
    fn format_interactive_elements(json_result: &str, filter_type: &str, scope: &str) -> Result<String, String> {
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
            let scope_label = if scope == "viewport" { "current viewport" } else { "page" };
            return Ok(format!("No {} elements found in {}.", filter_label, scope_label));
        }

        // Header with metadata
        let filter_label = filter_type.replace('_', " ");
        let scope_label = if scope == "viewport" { "viewport" } else { "page" };
        let mut output = format!("Found {} {} element(s) in {}:\n\n", elements.len(), filter_label, scope_label);

        // Format each element
        for el in &elements {
            // Format attributes
            let attrs: Vec<String> = el.attributes
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

            output.push_str(&format!("[{}] <{}{}>{}\n", el.index, el.tag, attr_str, text_str));
            output.push_str(&format!("    Selector: {}\n\n", el.selector));
        }

        // Footer with usage hint
        output.push_str("💡 Use the selector or index to interact with these elements.");

        Ok(output)
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
                *Use `createSession` to start*".to_string(),
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
                description: "Get the text content of the current page.".to_string(),
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
                service.close_session(&id).await?;
                {
                    let mut lock = self.browser_session_id.write().map_err(|e| e.to_string())?;
                    *lock = None;
                }
                return Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text {
                        text: "Browser session closed".to_string(),
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                });
            } else {
                return Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text {
                        text: "No active browser session to close".to_string(),
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                });
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
                    return Ok(MCPResult {
                        content: Some(vec![crate::mcp::types::MCPContent::Text {
                            text: format!("Session already exists: {}", id),
                        }]),
                        structured_content: None,
                        is_error: Some(false),
                    });
                }
            }

            let (id, status_msg) = service
                .create_browser_session(url, Some(&format!("Agent {}", self.agent_session_id)))
                .await?;

            {
                let mut id_lock = self.browser_session_id.write().map_err(|e| e.to_string())?;
                *id_lock = Some(id.clone());
            }

            return Ok(MCPResult {
                content: Some(vec![crate::mcp::types::MCPContent::Text {
                    text: format!("Browser session created: {}. {}", id, status_msg),
                }]),
                structured_content: None,
                is_error: Some(false),
            });
        }

        match tool_name {
            "navigateToUrl" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing url argument")?;
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;

                let result = service.navigate_to_url(session_id, url).await?;
                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "navigateBack" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let result = service.navigate_back(session_id).await?;
                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "navigateForward" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let result = service.navigate_forward(session_id).await?;
                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "getCurrentUrl" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let result = service
                    .execute_script(session_id, "window.location.href")
                    .await?;
                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "getPageTitle" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let result = service.execute_script(session_id, "document.title").await?;
                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "extractWebContent" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                // Script to extract text content
                // Note: Legacy used 'document.body.innerText', but we might want a better extraction later
                let script = "document.body.innerText";
                let result = service.execute_script(session_id, script).await?;
                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "clickElement" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing selector")?;

                let script = Self::get_click_script(selector);
                let result = service.execute_script(session_id, &script).await?;

                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "inputText" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let selector = args
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing selector")?;
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing text")?;

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

                let result = service.execute_script(session_id, &script).await?;

                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "scrollPage" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let x = args.get("x").and_then(|v| v.as_f64()).ok_or("Missing x")?;
                let y = args.get("y").and_then(|v| v.as_f64()).ok_or("Missing y")?;

                let script = format!("window.scrollTo({}, {}); 'Scrolled'", x, y);
                let result = service.execute_script(session_id, &script).await?;

                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "listInteractable" => {
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let filter_type = args
                    .get("filterType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("semantic_clickable");
                let scope = args
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or("viewport");

                let script = Self::get_filter_script(filter_type, scope);
                let result_json = service.execute_script(session_id, &script).await?;

                // Parse and format results to match TypeScript version
                let formatted_text = BrowserServer::format_interactive_elements(&result_json, filter_type, scope)?;

                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: formatted_text }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "inject_javascript" => {
                let script = args
                    .get("script")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing script argument")?;
                let session_id = args
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing sessionId")?;
                let result = service.execute_script(session_id, script).await?;
                Ok(MCPResult {
                    content: Some(vec![crate::mcp::types::MCPContent::Text { text: result }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
