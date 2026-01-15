use crate::mcp::builtin::error_guidance::{operation_failed_error, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::mcp::MCPTool;
use crate::services::InteractiveBrowserServer;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Manager};

use super::BuiltinMCPServer;

mod content;
mod interaction;
mod navigation;
mod session;

/// A built-in MCP server that constructs a bridge to the InteractiveBrowserServer service
///
/// # Browser Tool Workflows
///
/// ## Basic Navigation Flow
/// 1. `createSession(url?)` → get `session_id`
/// 2. `navigateToUrl(session_id, url)` → navigate to page
/// 3. `extractWebContent(session_id)` → read content
///
/// ## Interaction Flow
/// 1. `listInteractable(session_id)` → find elements
/// 2. `clickElement(session_id, selector)` → interact
/// 3. `extractWebContent(session_id)` → verify changes
///
/// ## Error Recovery
/// - **Session expired**: call `createSession` again
/// - **Element not found**: use `listInteractable` to find valid selectors
/// - **Page load timeout**: abandon and try different URL
/// - **HTTP 403/401**: do NOT retry, search alternative sources
#[derive(Debug)]
pub struct BrowserServer {
    pub(crate) app_handle: AppHandle,
    pub(crate) agent_session_id: String,
    // We keep track of the browser session ID associated with this agent session
    pub(crate) browser_session_id: Arc<RwLock<Option<String>>>,
    // Cache for browser state to avoid expensive JS injection on every context request
    // Format: (url, title, last_update_timestamp)
    pub(crate) state_cache: Arc<RwLock<Option<(String, String, std::time::Instant)>>>,
}

pub(crate) fn handle_browser_op_error(
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

impl BrowserServer {
    pub fn new(app_handle: AppHandle, agent_session_id: String) -> Self {
        Self {
            app_handle,
            agent_session_id,
            browser_session_id: Arc::new(RwLock::new(None)), // Initialize lazily
            state_cache: Arc::new(RwLock::new(None)),        // Initialize cache as empty
        }
    }

    /// Get the browser service from Tauri state
    pub(crate) fn get_browser_service(&self) -> Result<InteractiveBrowserServer, String> {
        self.app_handle
            .try_state::<InteractiveBrowserServer>()
            .map(|s| s.inner().clone())
            .ok_or_else(|| "InteractiveBrowserServer state not found".to_string())
    }

    /// Invalidate state cache (call after navigation or page changes)
    pub(crate) fn invalidate_cache(&self) {
        if let Ok(mut cache_guard) = self.state_cache.write() {
            *cache_guard = None;
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for BrowserServer {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Tools for interacting with a web browser."
    }

    async fn get_service_context(
        &self,
        _options: Option<&Value>,
    ) -> crate::mcp::types::ServiceContext {
        // Check if browser session exists
        let browser_session_id = match self.browser_session_id.read() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };

        let session_id = match browser_session_id {
            Some(id) => id,
            None => {
                return crate::mcp::types::ServiceContext {
                    context_prompt: "## Browser\n\nNo active session".to_string(),
                    structured_state: Some(json!({
                        "active": false
                    })),
                };
            }
        };

        // Check cache first (5 second TTL to avoid expensive JS injection)
        const CACHE_TTL_SECS: u64 = 5;
        if let Ok(cache_guard) = self.state_cache.read() {
            if let Some((cached_url, cached_title, last_update)) = cache_guard.as_ref() {
                let elapsed = last_update.elapsed();
                if elapsed.as_secs() < CACHE_TTL_SECS {
                    // Use cached data with full session_id
                    let context_prompt = format!(
                        "## Browser\n\nSession {}: {} ({})",
                        session_id, cached_url, cached_title
                    );

                    return crate::mcp::types::ServiceContext {
                        context_prompt,
                        structured_state: Some(json!({
                            "active": true,
                            "session_id": session_id,
                            "url": cached_url,
                            "title": cached_title,
                            "cached": true
                        })),
                    };
                }
            }
        }

        // Cache miss or expired - fetch fresh data via JS injection
        let service = match self.get_browser_service() {
            Ok(s) => s,
            Err(_) => {
                return crate::mcp::types::ServiceContext {
                    context_prompt: "## Browser\n\nService unavailable".to_string(),
                    structured_state: Some(json!({
                        "active": false,
                        "error": "service_unavailable"
                    })),
                };
            }
        };

        // Get current URL
        let url = match service
            .execute_script(&session_id, "window.location.href")
            .await
        {
            Ok(result) => result.trim_matches('"').to_string(),
            Err(_) => "unknown".to_string(),
        };

        // Get page title
        let title = match service.execute_script(&session_id, "document.title").await {
            Ok(result) => result.trim_matches('"').to_string(),
            Err(_) => "unknown".to_string(),
        };

        // Update cache with fresh data
        if let Ok(mut cache_guard) = self.state_cache.write() {
            *cache_guard = Some((url.clone(), title.clone(), std::time::Instant::now()));
        }

        // Use full session_id so AI can call browser tools with correct ID
        let context_prompt = format!("## Browser\n\nSession {}: {} ({})", session_id, url, title);

        crate::mcp::types::ServiceContext {
            context_prompt,
            structured_state: Some(json!({
                "active": true,
                "session_id": session_id,
                "url": url,
                "title": title,
                "cached": false
            })),
        }
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "createSession".to_string(),
                description: "Create a new browser session for this agent.

⚠️ WORKFLOW:
1. Call createSession FIRST before any other browser operations
2. Use the returned session ID for all subsequent browser tools
3. Session automatically closes if agent terminates

Returns: Session ID (e.g., 'abc123...') - use this ID for all other browser tools".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                         "url": {
                            "type": "string",
                            "description": "Initial URL to open (default: about:blank)"
                        }
                    }
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "navigateToUrl".to_string(),
                description: "Navigate to a specific URL in the browser session.

The browser session is managed automatically by the backend. Simply provide the URL and the system will handle the navigation.

⚠️ Error Handling:
- 403/401: Page blocks automated access - abandon and search elsewhere
- 404: Page not found - check URL or search homepage
- Timeout: Page too complex or blocking - try different URL

Next Steps:
- Use extractWebContent to read page content
- Use listInteractable to see clickable elements".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "URL to navigate to (must start with http:// or https://)"
                        }
                    },
                    "required": ["url"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "navigateBack".to_string(),
                description: "Navigate back in browser history to the previous page.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "navigateForward".to_string(),
                description: "Navigate forward in browser history to the next page.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "getCurrentUrl".to_string(),
                description: "Get the current URL of the page.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "getPageTitle".to_string(),
                description: "Get the title of the current page.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "extractWebContent".to_string(),
                description: "Extract the content of the current page as markdown. Large pages are automatically paginated.

For pages > 3000 tokens, content is split into pages. Use readWebContent(page) to read subsequent pages.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "autoMerge": {
                            "type": "boolean",
                            "description": "Whether to attempt merging all pages into one response (default: true)."
                        },
                        "saveRawHtml": {
                            "type": "boolean",
                            "description": "Whether to save raw HTML to a file for debugging (default: false)"
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
                name: "clickElement".to_string(),
                description: "Click an element on the page using a CSS selector.

⚠️ PREREQUISITE:
- Call listInteractable OR extractWebContent FIRST to find valid selectors on the page.
- Do NOT guess selectors.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "selector": {
                            "type": "string",
                            "description": "CSS selector of the element to click (must match an element visible in listInteractable/extractWebContent)"
                        }
                    },
                    "required": ["selector"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "inputText".to_string(),
                description: "Input text into an element on the page.

⚠️ PREREQUISITE:
- Call listInteractable OR extractWebContent FIRST to find valid selectors.
- Verify the element is an input or textarea.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "selector": {
                            "type": "string",
                            "description": "CSS selector of the input element"
                        },
                        "text": {
                            "type": "string",
                            "description": "Text to input"
                        }
                    },
                    "required": ["selector", "text"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "scrollPage".to_string(),
                description: "Scroll the page to a specific position.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "x": {
                            "type": "number",
                            "description": "X coordinate to scroll to"
                        },
                        "y": {
                            "type": "number",
                            "description": "Y coordinate to scroll to"
                        }
                    },
                    "required": ["x", "y"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },

            MCPTool {
                name: "listInteractable".to_string(),
                description: "List interactable elements on the page.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "filterType": {
                            "type": "string",
                            "enum": ["semantic_clickable", "semantic_input", "all_focusable"],
                            "description": "Filter type:\n- semantic_clickable: Buttons, links, and clickable elements\n- semantic_input: Inputs, textareas, and form fields\n- all_focusable: Everything that can receive focus"
                        },
                        "scope": {
                            "type": "string",
                            "enum": ["viewport", "all"],
                            "description": "Scope of listing (default: viewport)"
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
                name: "closeSession".to_string(),
                description: "Explicitly close the browser session. Good practice after finishing task to free resources.".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {}
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "readWebContent".to_string(),
                description: "Read a specific page from previously extracted content (if paginated).".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "page": {
                            "type": "number",
                            "description": "Page number to read (1-based index)"
                        }
                    },
                    "required": ["page"]
                }))
                .unwrap(),
                title: None,
                output_schema: None,
                annotations: None,
            },
        ]
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "createSession" => session::create_session(self, args).await,
            "closeSession" => session::close_session(self, args).await,
            "navigateToUrl" => navigation::navigate_to_url(self, args).await,
            "navigateBack" => navigation::navigate_back(self, args).await,
            "navigateForward" => navigation::navigate_forward(self, args).await,
            "getCurrentUrl" => navigation::get_current_url(self, args).await,
            "getPageTitle" => navigation::get_page_title(self, args).await,
            "extractWebContent" => content::extract_web_content(self, args).await,
            "readWebContent" => content::read_web_content(self, args).await,
            "clickElement" => interaction::click_element(self, args).await,
            "inputText" => interaction::input_text(self, args).await,
            "scrollPage" => interaction::scroll_page(self, args).await,
            "listInteractable" => interaction::list_interactable(self, args).await,

            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
