use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::builtin::BuiltinMCPServer;
use super::error_normalization::{external_tool_error_result, ExternalMcpErrorCategory};
use super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::types::{MCPResponse, MCPTool, ServiceContext};
use crate::session::SessionManager;

pub mod builder;
pub mod factory;
pub mod routing;
pub mod types;

pub use builder::MCPServiceProxyBuilder;
use factory::create_builtin_server;
use routing::{route_tool, ToolRouting};
pub use types::{ProxyConfig, SessionManagers, SharedManagers};

/// Session-specific MCP service proxy
///
/// Each proxy instance is bound to a single agent session and holds dedicated
/// instances of builtin MCP servers. This ensures complete isolation of tool
/// state and context across concurrent sessions.
#[derive(Debug)]
pub struct MCPServiceProxy {
    /// The session this proxy is bound to
    session_id: String,

    /// Session-specific builtin server instances
    /// Key: tool_id (e.g., "knowledge", "planning")
    /// Value: Boxed trait object implementing BuiltinMCPServer
    builtin_servers: HashMap<String, Box<dyn BuiltinMCPServer>>,

    /// Cached tools from session-isolated stdio servers
    /// Key: server_name, Value: list of tools
    /// This cache is populated during session creation (eager tool discovery)
    session_stdio_tool_cache: Arc<RwLock<HashMap<String, Vec<MCPTool>>>>,

    /// Cached tools from session-isolated HTTP servers
    /// Key: server_name, Value: list of tools
    session_http_tool_cache: Arc<RwLock<HashMap<String, Vec<MCPTool>>>>,

    /// Session-specific managers
    session_managers: SessionManagers,

    /// Tool execution timeout in seconds
    tool_timeout_seconds: u64,
}

impl MCPServiceProxy {
    fn tool_call_response(result: super::types::MCPResult) -> MCPResponse {
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(super::types::JsonRpcId::String(
                uuid::Uuid::new_v4().to_string(),
            )),
            result: Some(super::types::MCPResponseResult::ToolCall(result)),
            error: None,
        }
    }

    fn builtin_timeout_response(
        &self,
        server_id: &str,
        tool_name: &str,
    ) -> Result<MCPResponse, String> {
        let text = format!(
            "Tool {}__{} timed out after {} seconds.\n\nThis reflects a backend execution timeout, not invalid tool usage.\n\nYou can retry if the action is safe to repeat, or use a tool-specific status/check command if one exists.",
            server_id, tool_name, self.tool_timeout_seconds
        );

        Ok(Self::tool_call_response(
            super::types::MCPResult::informational_with_data(
                &text,
                serde_json::json!({
                    "timeout": true,
                    "server": server_id,
                    "tool": tool_name,
                    "sessionId": self.session_id,
                    "seconds": self.tool_timeout_seconds,
                }),
            ),
        ))
    }

    /// Create a new session-bound proxy using builder
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for the agent session
    /// * `external_mcp_manager` - Shared manager for external MCP servers
    /// * `db` - Shared SeaORM database connection
    /// * `session_manager` - Shared SessionManager for workspace/attachments
    /// * `http_manager` - Session-specific HTTP manager
    /// * `stdio_manager` - Session-specific Stdio manager
    ///
    /// # Returns
    /// * `MCPServiceProxyBuilder` - Builder to configure additional options
    pub fn builder(
        session_id: String,
        db: Arc<DatabaseConnection>,
        session_manager: Arc<SessionManager>,
        http_manager: Arc<HttpSessionManager>,
        stdio_manager: Arc<SessionMCPManager>,
    ) -> MCPServiceProxyBuilder {
        MCPServiceProxyBuilder::new(session_id, db, session_manager, http_manager, stdio_manager)
    }

    /// Internal method to create the proxy (used by builder)
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn create(
        session_id: String,
        tool_ids: Vec<String>,
        db: Arc<DatabaseConnection>,
        session_manager: Arc<SessionManager>,
        app_handle: Option<AppHandle>,
        http_manager: Arc<HttpSessionManager>,
        stdio_manager: Arc<SessionMCPManager>,
        tool_timeout_seconds: u64,
    ) -> Result<Self, String> {
        let mut builtin_servers = HashMap::new();

        for tool_id in &tool_ids {
            if let Some(server) = create_builtin_server(
                tool_id,
                session_id.clone(),
                db.clone(),
                session_manager.clone(),
                app_handle.clone(),
            )
            .await?
            {
                builtin_servers.insert(tool_id.to_string(), server);
                log::debug!(
                    "Initialized builtin server '{}' for session '{}'",
                    tool_id,
                    session_id
                );
            } else {
                log::warn!(
                    "Unknown builtin tool ID '{}' requested for session '{}'",
                    tool_id,
                    session_id
                );
            }
        }

        Ok(Self {
            session_id,
            builtin_servers,
            session_stdio_tool_cache: Arc::new(RwLock::new(HashMap::new())),
            session_http_tool_cache: Arc::new(RwLock::new(HashMap::new())),
            session_managers: SessionManagers {
                http: http_manager,
                stdio: stdio_manager,
            },
            tool_timeout_seconds,
        })
    }

    /// Call a tool through this proxy
    ///
    /// Routes the call to either:
    /// - Builtin server (if tool_name's service prefix matches a known builtin service)
    /// - External MCP server (stdio-based)
    ///
    /// # Arguments
    /// * `tool_name` - Full tool name (e.g., "attachments__add")
    /// * `args` - JSON arguments for the tool
    ///
    /// # Returns
    /// * `Ok(MCPResponse)` - Tool execution result
    /// * `Err(String)` - Error if tool not found or execution fails
    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResponse, String> {
        // tool_timeout_seconds == 0 means timeout is disabled.
        // In that case run the future directly without a deadline so long-running
        // tools (e.g. builtin_swarm__awaitAgent) are never killed by the proxy.
        if self.tool_timeout_seconds == 0 {
            return match route_tool(tool_name)? {
                ToolRouting::Builtin {
                    server_id,
                    tool_name: real_tool_name,
                } => match self.builtin_servers.get(&server_id) {
                    Some(server) => {
                        log::debug!(
                            "Calling builtin tool '{}' for session '{}'",
                            tool_name,
                            self.session_id
                        );
                        let result = server
                            .call_tool(&real_tool_name, args, Some(self.session_id.clone()))
                            .await?;
                        Ok(Self::tool_call_response(result))
                    }
                    None => {
                        let available = self
                            .builtin_servers
                            .keys()
                            .map(|k| format!("'{}'", k))
                            .collect::<Vec<_>>()
                            .join(", ");
                        Err(format!(
                            "Built-in server '{}' not enabled in this session.\n\n\
                                    Available servers: [{}]\n\n\
                                    💡 To fix: Update the assistant's \
                                    'allowedBuiltInServiceAliases' configuration to include \
                                    \"{}\"",
                            server_id, available, server_id
                        ))
                    }
                },
                ToolRouting::External {
                    server_name,
                    tool_name: real_tool_name,
                } => {
                    log::debug!(
                        "Routing to external MCP: '{}' for session '{}'",
                        tool_name,
                        self.session_id
                    );

                    if let Some(response) = self
                        .try_dispatch_external(&server_name, &real_tool_name, args)
                        .await
                    {
                        return response;
                    }

                    let result = external_tool_error_result(
                        "Call External Tool",
                        &server_name,
                        &real_tool_name,
                        ExternalMcpErrorCategory::NotFound,
                        &format!(
                            "Tool '{}' not found in session '{}'",
                            tool_name, self.session_id
                        ),
                        vec![
                            "Verify the server is enabled for this agent/session".to_string(),
                            "Re-run session tool discovery to list available tools".to_string(),
                            "Confirm the tool name matches the server tool list".to_string(),
                        ],
                    );

                    Ok(Self::tool_call_response(result))
                }
            };
        }

        let timeout_duration = std::time::Duration::from_secs(self.tool_timeout_seconds);

        // Wrap the entire execution in a timeout
        tokio::time::timeout(timeout_duration, async {
            match route_tool(tool_name)? {
                ToolRouting::Builtin {
                    server_id,
                    tool_name: real_tool_name,
                } => {
                    match self.builtin_servers.get(&server_id) {
                        Some(server) => {
                            log::debug!(
                                "Calling builtin tool '{}' for session '{}'",
                                tool_name,
                                self.session_id
                            );
                            let result = server
                                .call_tool(&real_tool_name, args, Some(self.session_id.clone()))
                                .await?;
                            // Convert MCPResult to MCPResponse with proper type
                            Ok(Self::tool_call_response(result))
                        }
                        None => {
                            let available = self
                                .builtin_servers
                                .keys()
                                .map(|k| format!("'{}'", k))
                                .collect::<Vec<_>>()
                                .join(", ");
                            Err(format!(
                                "Built-in server '{}' not enabled in this session.\n\n\
                                    Available servers: [{}]\n\n\
                                    💡 To fix: Update the assistant's \
                                    'allowedBuiltInServiceAliases' configuration to include \
                                    \"{}\"",
                                server_id, available, server_id
                            ))
                        }
                    }
                }
                ToolRouting::External {
                    server_name,
                    tool_name: real_tool_name,
                } => {
                    // Route to external MCP manager or session-isolated manager
                    log::debug!(
                        "Routing to external MCP: '{}' for session '{}'",
                        tool_name,
                        self.session_id
                    );

                    if let Some(response) = self
                        .try_dispatch_external(&server_name, &real_tool_name, args)
                        .await
                    {
                        return response;
                    }

                    let result = external_tool_error_result(
                        "Call External Tool",
                        &server_name,
                        &real_tool_name,
                        ExternalMcpErrorCategory::NotFound,
                        &format!(
                            "Tool '{}' not found in session '{}'",
                            tool_name, self.session_id
                        ),
                        vec![
                            "Verify the server is enabled for this agent/session".to_string(),
                            "Re-run session tool discovery to list available tools".to_string(),
                            "Confirm the tool name matches the server tool list".to_string(),
                        ],
                    );

                    Ok(Self::tool_call_response(result))
                }
            }
        })
        .await
        .unwrap_or_else(|_| match route_tool(tool_name) {
            Ok(ToolRouting::Builtin {
                server_id,
                tool_name: real_tool_name,
            }) => self.builtin_timeout_response(&server_id, &real_tool_name),
            Ok(ToolRouting::External { .. }) => Err(format!(
                "Tool execution timed out after {} seconds",
                self.tool_timeout_seconds
            )),
            Err(e) => Err(e),
        })
    }

    /// Get the session ID this proxy is bound to
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get list of available builtin tool IDs
    pub fn builtin_tool_ids(&self) -> Vec<String> {
        self.builtin_servers.keys().cloned().collect()
    }

    /// Get the number of builtin servers
    pub fn builtin_server_count(&self) -> usize {
        self.builtin_servers.len()
    }

    /// Get tools from a specific builtin server
    pub fn get_builtin_server_tools(&self, server_id: &str) -> Vec<super::types::MCPTool> {
        self.builtin_servers
            .get(server_id)
            .map(|server| {
                server
                    .tools()
                    .into_iter()
                    .map(|mut tool| {
                        // Normalize tool name to include builtin prefix and server ID
                        // This ensures the orchestrator can correctly route the tool call back to this proxy
                        // format: builtin_{server_id}__{tool_name}
                        tool.name = format!("{}__{}", server_id, &tool.name);
                        tool
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Attempts to dispatch a tool call to an external MCP server (HTTP or Stdio).
    ///
    /// # Returns
    /// * `Some(result)` if the server was found and the call was dispatched
    /// * `None` if no external server with that name is registered in this session
    async fn try_dispatch_external(
        &self,
        server_name: &str,
        real_tool_name: &str,
        args: Value,
    ) -> Option<Result<MCPResponse, String>> {
        if self.session_managers.http.has_server(server_name).await {
            log::debug!("Routing to session-isolated HTTP server: {}", server_name);
            let response = match self
                .session_managers
                .http
                .call_tool(server_name, real_tool_name, args)
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    let result = external_tool_error_result(
                        "Call External Tool",
                        server_name,
                        real_tool_name,
                        ExternalMcpErrorCategory::Transport,
                        &e.to_string(),
                        vec![
                            "Verify the HTTP MCP server URL and headers are valid".to_string(),
                            "If this server is session-scoped, ensure it is enabled for this agent/session".to_string(),
                            "Re-run session tool discovery to confirm tool availability".to_string(),
                        ],
                    );
                    MCPResponse {
                        jsonrpc: "2.0".to_string(),
                        id: Some(super::types::JsonRpcId::String(
                            uuid::Uuid::new_v4().to_string(),
                        )),
                        result: Some(super::types::MCPResponseResult::ToolCall(result)),
                        error: None,
                    }
                }
            };
            return Some(Ok(response));
        }

        if self.session_managers.stdio.has_server(server_name) {
            log::debug!("Routing to session-isolated Stdio server: {}", server_name);
            let response = match self
                .session_managers
                .stdio
                .call_tool(server_name, real_tool_name, args)
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    let result = external_tool_error_result(
                        "Call External Tool",
                        server_name,
                        real_tool_name,
                        ExternalMcpErrorCategory::Transport,
                        &e.to_string(),
                        vec![
                            "Verify the MCP server command can be spawned".to_string(),
                            "Check server stderr logs for startup errors".to_string(),
                            "Re-run session tool discovery to confirm tool availability"
                                .to_string(),
                        ],
                    );
                    MCPResponse {
                        jsonrpc: "2.0".to_string(),
                        id: Some(super::types::JsonRpcId::String(
                            uuid::Uuid::new_v4().to_string(),
                        )),
                        result: Some(super::types::MCPResponseResult::ToolCall(result)),
                        error: None,
                    }
                }
            };
            return Some(Ok(response));
        }

        None
    }

    /// Get cached tools from session-isolated stdio servers
    ///
    /// Returns tools that were fetched during session creation (eager tool discovery).
    /// These tools are from stdio servers that are spawned per-session for isolation.
    ///
    /// # Returns
    /// * `Vec<MCPTool>` - All cached tools from all session stdio servers
    pub async fn get_session_stdio_tools(&self) -> Vec<MCPTool> {
        let cache = self.session_stdio_tool_cache.read().await;
        cache.values().flatten().cloned().collect()
    }

    /// Set cached tools for a specific session-isolated stdio server
    ///
    /// This is called during session creation to store tools fetched from
    /// eagerly-spawned stdio servers.
    ///
    /// # Arguments
    /// * `server_name` - Name of the stdio server
    /// * `tools` - List of tools from that server
    pub async fn set_session_stdio_tools(&self, server_name: String, tools: Vec<MCPTool>) {
        let mut cache = self.session_stdio_tool_cache.write().await;
        cache.insert(server_name, tools);
    }

    /// Get cached tools from session-isolated HTTP servers
    pub async fn get_session_http_tools(&self) -> Vec<MCPTool> {
        let cache = self.session_http_tool_cache.read().await;
        cache.values().flatten().cloned().collect()
    }

    /// Set cached tools for a specific session-isolated HTTP server
    pub async fn set_session_http_tools(&self, server_name: String, tools: Vec<MCPTool>) {
        let mut cache = self.session_http_tool_cache.write().await;
        cache.insert(server_name, tools);
    }

    /// Check if HTTP tools are cached for a specific server in this session
    ///
    /// # Arguments
    /// * `server_name` - Name of the HTTP server to check
    ///
    /// # Returns
    /// * `bool` - true if tools are cached, false otherwise
    pub async fn has_http_tools_cached(&self, server_name: &str) -> bool {
        let cache = self.session_http_tool_cache.read().await;
        cache.contains_key(server_name)
    }

    /// Get session-specific HTTP manager
    pub fn get_http_manager(&self) -> &Arc<HttpSessionManager> {
        &self.session_managers.http
    }

    /// Get session-specific Stdio manager
    pub fn get_stdio_manager(&self) -> &Arc<SessionMCPManager> {
        &self.session_managers.stdio
    }

    /// Call a builtin server method directly through the session-bound proxy instance.
    ///
    /// This is the **internal UI-only write path** for operations that must not be
    /// exposed as agent tools (e.g. add/delete attachment). Bypassing the agent-visible
    /// tool surface while still routing through the same session-bound server instance
    /// ensures that `recent_uploads` tracking, BM25 search index, and all other
    /// in-memory state are kept in sync with subsequent agent reads (`list`,
    /// `search`).
    ///
    /// # Arguments
    /// * `server_id` - Builtin server key (e.g. `"attachments"`)
    /// * `method` - Method name dispatched to `BuiltinMCPServer::call_tool` (e.g. `"add"`)
    /// * `args`   - JSON arguments forwarded verbatim
    ///
    /// # Errors
    /// Returns `Err` if the server is not registered for this session, or if the
    /// underlying `call_tool` returns an error.
    pub async fn call_builtin_internal(
        &self,
        server_id: &str,
        method: &str,
        args: Value,
    ) -> Result<super::types::MCPResult, String> {
        let server = self.builtin_servers.get(server_id).ok_or_else(|| {
            let available = self
                .builtin_servers
                .keys()
                .map(|k| format!("'{}'", k))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Built-in server '{}' not enabled in this session (available: [{}])",
                server_id, available
            )
        })?;

        server
            .call_tool(method, args, Some(self.session_id.clone()))
            .await
    }

    /// Collect service contexts from all builtin servers
    ///
    /// Queries all registered builtin servers in parallel. Servers that report
    /// no active state via `has_active_state()` are skipped to avoid
    /// unnecessary DB round-trips when context would be empty.
    ///
    /// # Returns
    /// * `HashMap<String, ServiceContext>` - Map of tool_id -> ServiceContext
    pub async fn get_service_contexts(
        &self,
        assistant_id: Option<&str>,
    ) -> HashMap<String, ServiceContext> {
        let options = serde_json::to_value(crate::mcp::types::ServiceContextOptions {
            session_id: Some(self.session_id.clone()),
            assistant_id: assistant_id.map(str::to_string),
        })
        .ok();

        let futures: Vec<_> = self
            .builtin_servers
            .iter()
            .map(|(tool_id, server)| {
                let tool_id = tool_id.clone();
                let options = options.clone();
                async move {
                    if !server.has_active_state().await {
                        return None;
                    }
                    let context = server.get_service_context(options.as_ref()).await;
                    Some((tool_id, context))
                }
            })
            .collect();

        let contexts: HashMap<String, ServiceContext> = futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        log::debug!(
            "Collected {} service contexts for session '{}' (parallel)",
            contexts.len(),
            self.session_id
        );

        contexts
    }
}
