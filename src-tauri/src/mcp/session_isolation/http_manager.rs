use log::{debug, info};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::mcp::server::MCPServerManager;
use crate::mcp::types::{MCPResponse, MCPServerConfig};

use super::error::SessionMCPError;

/// Manages HTTP/SSE MCP server connections with session context injection.
///
/// Unlike stdio servers which need isolated processes, HTTP servers are shared
/// across sessions but inject the session ID via the Mcp-Session-Id header.
pub struct HttpSessionManager {
    /// Unique session identifier
    session_id: String,

    /// Shared MCP server manager (contains HTTP connections)
    server_manager: Arc<MCPServerManager>,

    /// Map of server names to their configurations (HTTP only)
    http_configs: Arc<RwLock<HashMap<String, MCPServerConfig>>>,
}

impl HttpSessionManager {
    /// Creates a new HTTP session manager for the given session.
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for this agent session
    /// * `server_manager` - Shared MCP server manager with HTTP connections
    /// * `http_configs` - HTTP server configurations
    pub fn new(
        session_id: String,
        server_manager: Arc<MCPServerManager>,
        http_configs: HashMap<String, MCPServerConfig>,
    ) -> Self {
        info!(
            "Created HTTP session manager for session '{}' with {} servers",
            session_id,
            http_configs.len()
        );

        Self {
            session_id,
            server_manager,
            http_configs: Arc::new(RwLock::new(http_configs)),
        }
    }

    /// Calls a tool on an HTTP MCP server with session context.
    ///
    /// The session ID is automatically injected via the Mcp-Session-Id header
    /// when the server connection was established.
    ///
    /// # Arguments
    /// * `server_name` - Name of the HTTP server
    /// * `tool_name` - Name of the tool to call
    /// * `args` - JSON arguments for the tool
    ///
    /// # Returns
    /// * `Ok(MCPResponse)` - Tool execution result
    /// * `Err(SessionMCPError)` - Error if server not found or call fails
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<MCPResponse, SessionMCPError> {
        // Verify server is in our HTTP configs
        let configs = self.http_configs.read().await;
        if !configs.contains_key(server_name) {
            return Err(SessionMCPError::ServerNotFound(server_name.to_string()));
        }
        drop(configs);

        debug!(
            "Calling HTTP tool '{}::{}' for session '{}'",
            server_name, tool_name, self.session_id
        );

        // Call through shared server manager
        // The Mcp-Session-Id header was already set when the connection was created
        let response = self
            .server_manager
            .call_tool(server_name, tool_name, args, None)
            .await;

        Ok(response)
    }

    /// Get the list of HTTP server names managed by this session.
    pub async fn list_servers(&self) -> Vec<String> {
        self.http_configs.read().await.keys().cloned().collect()
    }

    /// Check if a specific server is managed by this session.
    pub async fn has_server(&self, server_name: &str) -> bool {
        self.http_configs.read().await.contains_key(server_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_manager_creation() {
        let session_id = "test-session".to_string();
        let session_manager = Arc::new(crate::session::SessionManager::new().unwrap());
        let server_manager = Arc::new(MCPServerManager::new_with_session_manager(session_manager));
        let http_configs = HashMap::new();

        let manager = HttpSessionManager::new(session_id.clone(), server_manager, http_configs);

        assert_eq!(manager.session_id, session_id);
        assert_eq!(manager.list_servers().await.len(), 0);
    }

    #[tokio::test]
    async fn test_server_lookup() {
        let session_id = "test-session".to_string();
        let session_manager = Arc::new(crate::session::SessionManager::new().unwrap());
        let server_manager = Arc::new(MCPServerManager::new_with_session_manager(session_manager));

        let mut http_configs = HashMap::new();
        let config = MCPServerConfig {
            name: "test-server".to_string(),
            transport: crate::mcp::types::TransportConfig::Http {
                url: "http://localhost:3000".to_string(),
                protocol_version: "2025-06-18".to_string(),
                session_id: Some(session_id.clone()),
                headers: None,
                enable_sse: Some(true),
                security: None,
            },
            authentication: None,
            metadata: None,
        };
        http_configs.insert("test-server".to_string(), config);

        let manager = HttpSessionManager::new(session_id, server_manager, http_configs);

        assert!(manager.has_server("test-server").await);
        assert!(!manager.has_server("non-existent").await);
        assert_eq!(manager.list_servers().await, vec!["test-server"]);
    }
}
