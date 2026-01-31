use crate::repositories::SessionRepository;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::builtin::BuiltinMCPServer;
use super::server::MCPServerManager;
use super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::types::{MCPResponse, MCPTool, ServiceContext};
use crate::session::SessionManager;

/// Configuration for creating an MCPServiceProxy
#[derive(Debug)]
pub struct ProxyConfig {
    /// Unique identifier for the agent session
    pub session_id: String,
    /// List of builtin tool IDs to initialize
    pub tool_ids: Vec<String>,
    /// Optional Tauri app handle for builtin servers
    pub app_handle: Option<AppHandle>,
}

/// Shared manager dependencies for MCPServiceProxy
#[derive(Debug, Clone)]
pub struct SharedManagers {
    /// Shared manager for external MCP servers
    pub external_mcp: Arc<MCPServerManager>,
    /// Shared SeaORM database connection
    pub db: Arc<DatabaseConnection>,
    /// Shared SessionManager for workspace/content_store
    pub session_manager: Arc<SessionManager>,
}

/// Session-specific manager dependencies
#[derive(Debug, Clone)]
pub struct SessionManagers {
    /// Session-specific HTTP manager
    pub http: Arc<HttpSessionManager>,
    /// Session-specific Stdio manager
    pub stdio: Arc<SessionMCPManager>,
}

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
}

/// Builder for MCPServiceProxy
pub struct MCPServiceProxyBuilder {
    session_id: String,
    tool_ids: Vec<String>,
    // external_mcp_manager: Arc<MCPServerManager>, // Removed as we use session isolation
    db: Arc<DatabaseConnection>,
    session_manager: Arc<SessionManager>,
    app_handle: Option<AppHandle>,
    http_manager: Arc<HttpSessionManager>,
    stdio_manager: Arc<SessionMCPManager>,
}

impl MCPServiceProxyBuilder {
    /// Create a new builder with required fields
    pub fn new(
        session_id: String,
        db: Arc<DatabaseConnection>,
        session_manager: Arc<SessionManager>,
        http_manager: Arc<HttpSessionManager>,
        stdio_manager: Arc<SessionMCPManager>,
    ) -> Self {
        Self {
            session_id,
            tool_ids: Vec::new(),
            db,
            session_manager,
            app_handle: None,
            http_manager,
            stdio_manager,
        }
    }

    /// Set the tool IDs to initialize
    pub fn with_tool_ids(mut self, tool_ids: Vec<String>) -> Self {
        self.tool_ids = tool_ids;
        self
    }

    /// Set the app handle
    pub fn with_app_handle(mut self, app_handle: Option<AppHandle>) -> Self {
        self.app_handle = app_handle;
        self
    }

    /// Build the MCPServiceProxy
    pub async fn build(self) -> Result<MCPServiceProxy, String> {
        MCPServiceProxy::create(
            self.session_id,
            self.tool_ids,
            self.db,
            self.session_manager,
            self.app_handle,
            self.http_manager,
            self.stdio_manager,
        )
        .await
    }
}

impl MCPServiceProxy {
    /// Create a new session-bound proxy using builder
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for the agent session
    /// * `external_mcp_manager` - Shared manager for external MCP servers
    /// * `db` - Shared SeaORM database connection
    /// * `session_manager` - Shared SessionManager for workspace/content_store
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
    async fn create(
        session_id: String,
        tool_ids: Vec<String>,
        db: Arc<DatabaseConnection>,
        session_manager: Arc<SessionManager>,
        app_handle: Option<AppHandle>,
        http_manager: Arc<HttpSessionManager>,
        stdio_manager: Arc<SessionMCPManager>,
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
        })
    }

    /// Call a tool through this proxy
    ///
    /// Routes the call to either:
    /// - Builtin server (if tool_name starts with "builtin_")
    /// - External MCP server (stdio-based)
    ///
    /// # Arguments
    /// * `tool_name` - Full tool name (e.g., "builtin_content_store__addContent")
    /// * `args` - JSON arguments for the tool
    ///
    /// # Returns
    /// * `Ok(MCPResponse)` - Tool execution result
    /// * `Err(String)` - Error if tool not found or execution fails
    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResponse, String> {
        if tool_name.starts_with("builtin_") {
            // Extract tool ID from full name (builtin_content_store__addContent -> content_store)
            let tool_id = tool_name
                .strip_prefix("builtin_")
                .and_then(|s| s.split("__").next())
                .ok_or_else(|| format!("Invalid builtin tool name: {}", tool_name))?;

            let server = self
                .builtin_servers
                .get(tool_id)
                .ok_or_else(|| format!("Built-in server not found: {}", tool_id))?;

            log::debug!(
                "Calling builtin tool '{}' for session '{}'",
                tool_name,
                self.session_id
            );

            let result = {
                let prefix = format!("builtin_{}__", tool_id);
                let real_tool_name = tool_name.strip_prefix(&prefix).unwrap_or(tool_name);
                server
                    .call_tool(real_tool_name, args, Some(self.session_id.clone()))
                    .await?
            };

            // Convert MCPResult to MCPResponse with proper type
            Ok(MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(super::types::JsonRpcId::String(
                    uuid::Uuid::new_v4().to_string(),
                )),
                result: Some(super::types::MCPResponseResult::ToolCall(result)),
                error: None,
            })
        } else {
            // Route to external MCP manager or session-isolated manager
            // Format is typically "server_name__tool_name"
            log::debug!(
                "Routing to external MCP: '{}' for session '{}'",
                tool_name,
                self.session_id
            );

            if let Some((server_name, real_tool_name)) = tool_name.split_once("__") {
                // 1. Check if it's a session-isolated HTTP server
                if self.session_managers.http.has_server(server_name).await {
                    log::debug!("Routing to session-isolated HTTP server: {}", server_name);
                    return self
                        .session_managers
                        .http
                        .call_tool(server_name, real_tool_name, args)
                        .await
                        .map_err(|e| e.to_string());
                }

                // 2. Check if it's a session-isolated Stdio server
                if self.session_managers.stdio.has_server(server_name) {
                    log::debug!("Routing to session-isolated Stdio server: {}", server_name);
                    return self
                        .session_managers
                        .stdio
                        .call_tool(server_name, real_tool_name, args)
                        .await
                        .map_err(|e| e.to_string());
                }

                Err(format!(
                    "Tool '{}' not found in session '{}'. Session isolation is active, and the tool is not available in the session-specific server instances.",
                    tool_name, self.session_id
                ))
            } else {
                Err(format!(
                    "Invalid external tool name format: {}. Expected 'server__tool'",
                    tool_name
                ))
            }
        }
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
                        tool.name = format!("builtin_{}__{}", server_id, tool.name);
                        tool
                    })
                    .collect()
            })
            .unwrap_or_default()
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

    /// Get session-specific HTTP manager
    pub fn get_http_manager(&self) -> &Arc<HttpSessionManager> {
        &self.session_managers.http
    }

    /// Get session-specific Stdio manager
    pub fn get_stdio_manager(&self) -> &Arc<SessionMCPManager> {
        &self.session_managers.stdio
    }

    /// Collect service contexts from all builtin servers
    ///
    /// Iterates through all registered builtin servers and collects their
    /// current service context information. This is used to enrich the system
    /// prompt with real-time session state information.
    ///
    /// # Returns
    /// * `HashMap<String, ServiceContext>` - Map of tool_id -> ServiceContext
    pub async fn get_service_contexts(&self) -> HashMap<String, ServiceContext> {
        let mut contexts = HashMap::new();

        for (tool_id, server) in &self.builtin_servers {
            let context = server.get_service_context(None).await;

            // Always include the context, even if empty, as structured state might be present
            contexts.insert(tool_id.clone(), context);
        }

        log::debug!(
            "Collected {} service contexts for session '{}'",
            contexts.len(),
            self.session_id
        );

        contexts
    }
}

/// Factory function to create session-bound builtin server instances
///
/// This function is called during proxy initialization to create dedicated
/// server instances for the session.
///
/// # Arguments
/// * `tool_id` - The builtin tool identifier (e.g., "knowledge", "planning")
/// * `session_id` - The session to bind the server to
/// * `db` - Shared SeaORM database connection
///
/// # Returns
/// * `Ok(Some(Box<dyn BuiltinMCPServer>))` - Server instance
/// * `Ok(None)` - Unknown tool ID, skip
/// * `Err(String)` - Server initialization failed
async fn create_builtin_server(
    tool_id: &str,
    _session_id: String,
    _db: Arc<DatabaseConnection>,
    _session_manager: Arc<SessionManager>,
    app_handle: Option<AppHandle>,
) -> Result<Option<Box<dyn BuiltinMCPServer>>, String> {
    match tool_id {
        "bootstrap" => Ok(Some(Box::new(
            crate::mcp::builtin::bootstrap::BootstrapServer::new(),
        ))),
        "knowledge" => {
            let assistant_id = get_assistant_id_from_session(&_session_id).await?;
            Ok(Some(Box::new(
                crate::mcp::builtin::knowledge::KnowledgeServer::new(assistant_id, _db).await?,
            )))
        }
        "planning" => Ok(Some(Box::new(
            crate::mcp::builtin::planning::PlanningServer::new(_session_id, _db).await?,
        ))),
        "playbook" => Ok(Some(Box::new(
            crate::mcp::builtin::playbook::PlaybookServer::new(_session_id, _db).await?,
        ))),
        "assistant" => Ok(Some(Box::new(
            crate::mcp::builtin::assistant::AssistantServer::new(_db).await?,
        ))),
        "workspace" => Ok(Some(Box::new(
            crate::mcp::builtin::workspace::WorkspaceServer::new(_session_id, _session_manager),
        ))),
        "content_store" | "contentstore" => Ok(Some(Box::new(
            crate::mcp::builtin::content_store::ContentStoreServer::new(
                _session_id,
                _session_manager,
            ),
        ))),
        "ui" => Ok(Some(Box::new(crate::mcp::builtin::ui::UiServer::new()))),
        "browser" => {
            if let Some(handle) = app_handle {
                Ok(Some(Box::new(
                    crate::mcp::builtin::browser::BrowserServer::new(handle, _session_id),
                )))
            } else {
                log::warn!("Browser tool requested but no AppHandle provided (skipping)");
                Ok(None)
            }
        }
        "mcp_manager" => Ok(Some(Box::new(
            crate::mcp::builtin::mcp_manager::MCPManagerServer::new(),
        ))),
        "skills" => Ok(Some(Box::new(
            crate::mcp::builtin::skills::SkillsServer::new(_session_id),
        ))),
        _ => Ok(None), // Unknown tool, skip
    }
}

async fn get_assistant_id_from_session(session_id: &str) -> Result<String, String> {
    let session = crate::get_session_repository()
        .get_session(session_id)
        .await
        .map_err(|e| format!("Database error fetching session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let config_str = session
        .agent_config
        .clone()
        .ok_or_else(|| "Session has no config".to_string())?;

    let config: serde_json::Value = serde_json::from_str(&config_str)
        .map_err(|e| format!("Invalid session config JSON: {}", e))?;

    config
        .get("assistant_id")
        .or_else(|| config.get("assistantId"))
        .or_else(|| config.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No assistant ID in session config".to_string())
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[tokio::test]
    async fn test_builtin_tool_routing() {
        // TODO: Test that builtin_ prefix routes correctly
    }

    #[tokio::test]
    async fn test_external_tool_routing() {
        // TODO: Test that non-builtin tools route to external manager
    }

    #[tokio::test]
    async fn test_invalid_tool_name() {
        // TODO: Test error handling for invalid tool names
    }
}
