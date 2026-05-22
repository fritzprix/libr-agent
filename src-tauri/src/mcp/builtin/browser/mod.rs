use crate::mcp::builtin::browser_content_store::BrowserContentStore;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::{ContextVolatility, MCPResult};
use crate::mcp::MCPTool;
use crate::services::InteractiveBrowserServer;
use crate::services::SessionStatus;
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
/// 1. `createSession(url?)` → create or replace the active browser session for this agent
/// 2. `navigateToUrl(url)` → navigate the active session to a new page
/// 3. `getPageContent` → extract or read content from the current page
///
/// ## Interaction Flow
/// 1. `listInteractable` → find elements
/// 2. `clickElement(selector)` → interact
/// 3. `getPageContent` → verify changes
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
    pub(crate) content_store: BrowserContentStore,
}

pub(crate) fn handle_browser_op_error(
    operation: &str,
    error: String,
    default_guidance: Vec<&str>,
) -> MCPResult {
    let error_lower = error.to_lowercase();
    let is_timeout = error_lower.contains("timeout") || error_lower.contains("timed out");

    let category = if is_timeout {
        ErrorCategory::Timeout
    } else {
        ErrorCategory::OperationFailed
    };

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

    guided_error(
        category,
        format!("{} failed: {}", operation, error),
        ToolGroup::Browser,
    )
    .guidance(guidance)
    .to_mcp_result()
}

impl BrowserServer {
    pub fn new(app_handle: AppHandle, agent_session_id: String) -> Self {
        Self {
            app_handle,
            agent_session_id,
            browser_session_id: Arc::new(RwLock::new(None)), // Initialize lazily
            state_cache: Arc::new(RwLock::new(None)),        // Initialize cache as empty
            content_store: BrowserContentStore::new(),
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

pub const NAME: &str = "browser";

#[async_trait]
impl BuiltinMCPServer for BrowserServer {
    fn name(&self) -> &str {
        NAME
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
                return crate::mcp::types::ServiceContext::new(
                    "## Browser\n\n### Live State\n- No active session",
                )
                .with_structured_state(json!({
                    "active": false
                }))
                .with_volatility(ContextVolatility::Volatile);
            }
        };

        let service = match self.get_browser_service() {
            Ok(s) => s,
            Err(_) => {
                return crate::mcp::types::ServiceContext::new(
                    "## Browser\n\n### Live State\n- Service unavailable",
                )
                .with_structured_state(json!({
                    "active": false,
                    "error": "service_unavailable"
                }))
                .with_volatility(ContextVolatility::Volatile);
            }
        };

        let session = match service.get_session(&session_id) {
            Ok(session) => session,
            Err(_) => {
                return crate::mcp::types::ServiceContext::new(
                    "## Browser\n\n### Live State\n- Session expired or unavailable",
                )
                .with_structured_state(json!({
                    "active": false,
                    "error": "session_unavailable"
                }))
                .with_volatility(ContextVolatility::Volatile);
            }
        };

        // Check cache first (5 second TTL), but only for sessions that are still healthy.
        const CACHE_TTL_SECS: u64 = 5;
        if matches!(session.status, SessionStatus::Active) {
            if let Ok(cache_guard) = self.state_cache.read() {
                if let Some((cached_url, cached_title, last_update)) = cache_guard.as_ref() {
                    let elapsed = last_update.elapsed();
                    if elapsed.as_secs() < CACHE_TTL_SECS {
                        let context_prompt = format!(
                            "## Browser\n\n### Live State\n- Session: {}\n- Runtime: ready\n- URL: {}\n- Title: {}",
                            session_id, cached_url, cached_title
                        );

                        return crate::mcp::types::ServiceContext::new(context_prompt)
                            .with_structured_state(json!({
                                "active": true,
                                "session_id": session_id,
                                "url": cached_url,
                                "title": cached_title,
                                "runtime_state": "ready",
                                "runtime_ready": true,
                                "error": Value::Null,
                                "cached": true
                            }))
                            .with_volatility(ContextVolatility::Volatile);
                    }
                }
            }
        }

        let url = session.url.clone();
        let (runtime_state, title, error_message) = match &session.status {
            SessionStatus::Active => (
                "ready",
                session
                    .current_title
                    .clone()
                    .unwrap_or_else(|| "Untitled page".to_string()),
                None,
            ),
            SessionStatus::Error(message) => (
                "error",
                session
                    .current_title
                    .clone()
                    .unwrap_or_else(|| "Browser session error".to_string()),
                Some(message.clone()),
            ),
            _ => (
                "loading",
                session
                    .current_title
                    .clone()
                    .unwrap_or_else(|| "Loading...".to_string()),
                None,
            ),
        };

        // Update cache with fresh data
        if let Ok(mut cache_guard) = self.state_cache.write() {
            *cache_guard = Some((url.clone(), title.clone(), std::time::Instant::now()));
        }

        // Use full session_id so AI can call browser tools with correct ID
        let error_line = error_message
            .as_ref()
            .map(|message| format!("\n- Error: {}", message))
            .unwrap_or_default();
        let context_prompt = format!(
            "## Browser\n\n### Live State\n- Session: {}\n- Runtime: {}\n- URL: {}\n- Title: {}{}",
            session_id, runtime_state, url, title, error_line
        );

        crate::mcp::types::ServiceContext::new(context_prompt)
            .with_structured_state(json!({
                "active": true,
                "session_id": session_id,
                "url": url,
                "title": title,
                "runtime_state": runtime_state,
                "runtime_ready": session.is_runtime_ready(),
                "error": error_message,
                "cached": false
            }))
            .with_volatility(ContextVolatility::Volatile)
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "createSession" => session::create_session(self, args).await,
            "closeSession" => session::close_session(self, args).await,
            "navigateToUrl" => navigation::navigate_to_url(self, args).await,
            "navigateBack" => navigation::navigate_back(self, args).await,
            "navigateForward" => navigation::navigate_forward(self, args).await,
            "getCurrentUrl" => navigation::get_current_url(self, args).await,
            "getPageTitle" => navigation::get_page_title(self, args).await,
            "getPageContent" => content::smart_content(self, args).await,
            "clickElement" => interaction::click_element(self, args).await,
            "inputText" => interaction::input_text(self, args).await,
            "scrollPage" => interaction::scroll_page(self, args).await,
            "listInteractable" => interaction::list_interactable(self, args).await,
            "fetch" => content::fetch_url(self, args, session_id).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
        .or_else(|e| {
            if e.contains("cancelled") || e.contains("interrupted") {
                return Err(e);
            }
            Ok(guided_error(ErrorCategory::InternalError, e, ToolGroup::Browser).to_mcp_result())
        })
    }
}
