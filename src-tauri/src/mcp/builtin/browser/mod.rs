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
mod tools;

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
        if let Ok(mut cache_guard) = self.state_cache.write() {
            *cache_guard = None;
        }
    }

    /// Get metadata statically
    pub fn metadata_static() -> crate::mcp::types::BuiltinServerMetadata {
        crate::mcp::types::BuiltinServerMetadata {
            display_name: "Browser".to_string(),
            description: "Control and automate web browser interactions".to_string(),
            icon: None,
        }
    }
}

impl BrowserServer {
    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
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
        Self::tools_static()
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
