use anyhow::Result;
use log::info;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::mcp::types::{
    BuiltinServerInfo, MCPConnection, MCPResponse, MCPServerConfig, MCPTool, SamplingRequest,
    ServiceContext, ServiceContextOptions,
};
use crate::session::SessionManager;

mod lifecycle;
mod tools;

/// Manages the lifecycle and communication with both external and built-in MCP servers.
#[derive(Debug, Clone)]
pub struct MCPServerManager {
    /// A map of active connections to external MCP servers, keyed by server name.
    pub(crate) connections: Arc<Mutex<HashMap<String, MCPConnection>>>,
    /// A registry for the built-in MCP servers.
    pub(crate) builtin_servers: Arc<Mutex<Option<crate::mcp::builtin::BuiltinServerRegistry>>>,
    /// OAuth manager for handling OAuth 2.1 flows.
    pub(crate) oauth_manager: Arc<crate::mcp::oauth::OAuthManager>,
}

impl MCPServerManager {
    /// Creates a new `MCPServerManager` and initializes the built-in servers
    /// with a reference to the `SessionManager`.
    pub fn new_with_session_manager(session_manager: Arc<SessionManager>) -> Self {
        let server_manager = Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            builtin_servers: Arc::new(Mutex::new(None)),
            oauth_manager: Arc::new(crate::mcp::oauth::OAuthManager::new()),
        };

        // Initialize builtin servers immediately with SessionManager
        let builtin_registry =
            crate::mcp::builtin::BuiltinServerRegistry::new_with_session_manager(session_manager);
        *server_manager
            .builtin_servers
            .try_lock()
            .expect("Failed to initialize builtin servers") = Some(builtin_registry);
        info!("Initialized MCPServerManager with SessionManager-based builtin servers");

        server_manager
    }

    /// Creates a new `MCPServerManager` with support for both `SessionManager` and SQLite.
    pub async fn new_with_session_manager_and_sqlite(
        session_manager: Arc<SessionManager>,
        sqlite_db_url: String,
    ) -> Self {
        let server_manager = Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            builtin_servers: Arc::new(Mutex::new(None)),
            oauth_manager: Arc::new(crate::mcp::oauth::OAuthManager::new()),
        };

        // Initialize builtin servers with SessionManager and SQLite support
        let builtin_registry =
            crate::mcp::builtin::BuiltinServerRegistry::new_with_session_manager_and_sqlite(
                session_manager,
                sqlite_db_url,
            )
            .await;
        *server_manager
            .builtin_servers
            .try_lock()
            .expect("Failed to initialize builtin servers") = Some(builtin_registry);
        info!("Initialized MCPServerManager with SessionManager and SQLite support");

        server_manager
    }

    /// Creates a new `MCPServerManager` with support for both `SessionManager` and SeaORM DatabaseConnection.
    pub async fn new_with_session_manager_and_db(
        session_manager: Arc<SessionManager>,
        db: sea_orm::DatabaseConnection,
    ) -> Self {
        let server_manager = Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            builtin_servers: Arc::new(Mutex::new(None)),
            oauth_manager: Arc::new(crate::mcp::oauth::OAuthManager::new()),
        };

        // Initialize builtin servers with SessionManager and DatabaseConnection
        let builtin_registry =
            crate::mcp::builtin::BuiltinServerRegistry::new_with_session_manager_and_db(
                session_manager,
                db,
            )
            .await;
        *server_manager
            .builtin_servers
            .try_lock()
            .expect("Failed to initialize builtin servers") = Some(builtin_registry);
        info!("Initialized MCPServerManager with SessionManager and SeaORM DatabaseConnection");

        server_manager
    }

    /// Starts and connects to an MCP server based on the provided configuration.
    pub async fn start_server(&self, config: MCPServerConfig) -> Result<String> {
        lifecycle::start_server(self, config).await
    }

    /// Stops a running MCP server by name.
    pub async fn stop_server(&self, server_name: &str) -> Result<()> {
        lifecycle::stop_server(self, server_name).await
    }

    /// Performs text generation (sampling) on a specified MCP server.
    pub async fn sample_from_model(
        &self,
        server_name: &str,
        request: SamplingRequest,
        request_id: Option<serde_json::Value>,
    ) -> MCPResponse {
        tools::sample_from_model(self, server_name, request, request_id).await
    }

    /// Calls a tool on a specified MCP server with the given arguments.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
        request_id: Option<serde_json::Value>,
    ) -> MCPResponse {
        tools::call_tool(self, server_name, tool_name, arguments, request_id).await
    }

    /// Lists all tools available on a specific MCP server.
    pub async fn list_tools(&self, server_name: &str) -> Result<Vec<MCPTool>> {
        tools::list_tools(self, server_name).await
    }

    /// Lists all tools from all connected MCP servers.
    pub async fn list_all_tools(&self) -> Result<Vec<MCPTool>> {
        tools::list_all_tools(self).await
    }

    /// Returns a list of names of all currently connected external MCP servers.
    pub async fn get_connected_servers(&self) -> Vec<String> {
        tools::get_connected_servers(self).await
    }

    /// Checks if a specific external server is currently connected.
    pub async fn is_server_alive(&self, server_name: &str) -> bool {
        tools::is_server_alive(self, server_name).await
    }

    /// Checks the status of all connected external servers.
    pub async fn check_all_servers(&self) -> HashMap<String, bool> {
        tools::check_all_servers(self).await
    }

    /// Validates that a tool's input schema is compatible with AI service expectations.
    pub fn validate_tool_schema(tool: &MCPTool) -> Result<()> {
        tools::validate_tool_schema(tool)
    }

    /// Gets a list of tools from a server that pass schema validation.
    pub async fn get_validated_tools(&self, server_name: &str) -> Result<Vec<MCPTool>> {
        tools::get_validated_tools(self, server_name).await
    }

    /// Lists the names of all available built-in servers.
    pub async fn list_builtin_servers(&self) -> Vec<String> {
        tools::list_builtin_servers(self).await
    }

    /// Lists all tools from all available built-in servers.
    pub async fn list_builtin_tools(&self) -> Vec<MCPTool> {
        tools::list_builtin_tools(self).await
    }

    /// Lists the tools for a specific built-in server.
    pub async fn list_builtin_tools_for(&self, server_name: &str) -> Vec<MCPTool> {
        tools::list_builtin_tools_for(self, server_name).await
    }

    /// Lists all built-in servers with their metadata.
    pub async fn list_builtin_servers_with_metadata(&self) -> Vec<BuiltinServerInfo> {
        tools::list_builtin_servers_with_metadata(self).await
    }

    /// Lists all POSSIBLE builtin server definitions for UI configuration
    pub fn list_available_builtin_server_definitions() -> Vec<BuiltinServerInfo> {
        tools::list_available_builtin_server_definitions()
    }

    /// Calls a tool on a built-in server.
    pub async fn call_builtin_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
        request_id: Option<serde_json::Value>,
    ) -> MCPResponse {
        tools::call_builtin_tool(self, server_name, tool_name, args, request_id).await
    }

    /// Gets a unified list of all tools from both external and built-in servers.
    pub async fn list_all_tools_unified(&self) -> Result<Vec<MCPTool>> {
        tools::list_all_tools_unified(self).await
    }

    /// Calls a tool, automatically routing the request to either a built-in or an
    /// external server based on the server name prefix.
    pub async fn call_tool_unified(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
        request_id: Option<serde_json::Value>,
    ) -> MCPResponse {
        tools::call_tool_unified(self, server_name, tool_name, args, request_id).await
    }

    /// Gets the service context for a given server, checking built-in servers first.
    pub async fn get_service_context(
        &self,
        server_name: &str,
        options: Option<ServiceContextOptions>,
    ) -> Result<ServiceContext, String> {
        tools::get_service_context(self, server_name, options).await
    }

    /// Returns a reference to the OAuth manager for handling OAuth 2.1 flows.
    pub async fn get_oauth_manager(&self) -> Arc<crate::mcp::oauth::OAuthManager> {
        Arc::clone(&self.oauth_manager)
    }

    /// Check if a server uses stdio transport.
    ///
    /// This is used by the session isolation system to determine if a server
    /// needs per-session process management.
    pub async fn is_stdio_server(&self, server_name: &str) -> bool {
        let connections = self.connections.lock().await;
        connections
            .get(server_name)
            .map(|conn| {
                matches!(
                    conn.config.transport,
                    crate::mcp::types::TransportConfig::Stdio { .. }
                )
            })
            .unwrap_or(false)
    }

    /// Get all stdio server configurations.
    ///
    /// Returns a map of server name to configuration for all servers using stdio transport.
    /// This is used during session proxy creation to initialize session-specific managers.
    pub async fn get_stdio_configs(&self) -> HashMap<String, MCPServerConfig> {
        let connections = self.connections.lock().await;
        connections
            .iter()
            .filter(|(_, conn)| {
                matches!(
                    conn.config.transport,
                    crate::mcp::types::TransportConfig::Stdio { .. }
                )
            })
            .map(|(name, conn)| (name.clone(), conn.config.clone()))
            .collect()
    }

    /// Get transport configuration for a specific server.
    ///
    /// Returns None if the server is not found.
    pub async fn get_transport_config(
        &self,
        server_name: &str,
    ) -> Option<crate::mcp::types::TransportConfig> {
        let connections = self.connections.lock().await;
        connections
            .get(server_name)
            .map(|conn| conn.config.transport.clone())
    }

    /// Get all HTTP server configurations.
    ///
    /// Returns a map of server name to configuration for all servers using HTTP transport.
    /// This is used during session proxy creation to initialize session-specific HTTP managers.
    pub async fn get_http_configs(&self) -> HashMap<String, MCPServerConfig> {
        let connections = self.connections.lock().await;
        connections
            .iter()
            .filter(|(_, conn)| {
                matches!(
                    conn.config.transport,
                    crate::mcp::types::TransportConfig::Http { .. }
                )
            })
            .map(|(name, conn)| (name.clone(), conn.config.clone()))
            .collect()
    }
}
