use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

use super::builtin::BuiltinMCPServer;
use super::server::MCPServerManager;
use super::types::MCPResponse;
use crate::session::SessionManager;

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

    /// Shared external MCP manager for stdio-based servers
    external_mcp_manager: Arc<MCPServerManager>,

    /// Shared SessionManager for workspace/content_store servers
    _session_manager: Arc<SessionManager>,
}

impl MCPServiceProxy {
    /// Create a new session-bound proxy
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for the agent session
    /// * `tool_ids` - List of builtin tool IDs to initialize
    /// * `external_mcp_manager` - Shared manager for external MCP servers
    /// * `db_pool` - Shared SQLite connection pool
    /// * `session_manager` - Shared SessionManager for workspace/content_store
    ///
    /// # Returns
    /// * `Ok(Self)` - Initialized proxy with builtin servers
    /// * `Err(String)` - Error if server initialization fails
    pub async fn new(
        session_id: String,
        tool_ids: Vec<String>,
        external_mcp_manager: Arc<MCPServerManager>,
        db_pool: Arc<SqlitePool>,
        session_manager: Arc<SessionManager>,
    ) -> Result<Self, String> {
        let mut builtin_servers = HashMap::new();

        for tool_id in tool_ids {
            if let Some(server) = create_builtin_server(
                &tool_id,
                session_id.clone(),
                db_pool.clone(),
                session_manager.clone(),
            )
            .await?
            {
                builtin_servers.insert(tool_id.clone(), server);
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
            external_mcp_manager,
            _session_manager: session_manager,
        })
    }

    /// Call a tool through this proxy
    ///
    /// Routes the call to either:
    /// - Builtin server (if tool_name starts with "builtin_")
    /// - External MCP server (stdio-based)
    ///
    /// # Arguments
    /// * `tool_name` - Full tool name (e.g., "builtin_knowledge__saveKnowledge")
    /// * `args` - JSON arguments for the tool
    ///
    /// # Returns
    /// * `Ok(MCPResponse)` - Tool execution result
    /// * `Err(String)` - Error if tool not found or execution fails
    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResponse, String> {
        if tool_name.starts_with("builtin_") {
            // Extract tool ID from full name (builtin_knowledge__saveKnowledge -> knowledge)
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

            let result = server.call_tool(tool_name, args).await?;

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
            // Route to external MCP manager
            // Format is typically "server_name__tool_name"
            log::debug!(
                "Routing to external MCP: '{}' for session '{}'",
                tool_name,
                self.session_id
            );

            if let Some((server_name, real_tool_name)) = tool_name.split_once("__") {
                let response = self
                    .external_mcp_manager
                    .call_tool(server_name, real_tool_name, args, None)
                    .await;
                Ok(response)
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
            .map(|server| server.tools())
            .unwrap_or_default()
    }

    /// Collect service contexts from all builtin servers
    ///
    /// Iterates through all registered builtin servers and collects their
    /// current service context information. This is used to enrich the system
    /// prompt with real-time session state information.
    ///
    /// # Returns
    /// * `HashMap<String, String>` - Map of tool_id -> context_prompt
    pub async fn get_service_contexts(&self) -> HashMap<String, String> {
        let mut contexts = HashMap::new();

        for (tool_id, server) in &self.builtin_servers {
            let context = server.get_service_context(None).await;

            // Only include non-empty contexts
            if !context.context_prompt.trim().is_empty() {
                contexts.insert(tool_id.clone(), context.context_prompt);
            }
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
/// * `db_pool` - Shared SQLite connection pool
///
/// # Returns
/// * `Ok(Some(Box<dyn BuiltinMCPServer>))` - Server instance
/// * `Ok(None)` - Unknown tool ID, skip
/// * `Err(String)` - Server initialization failed
async fn create_builtin_server(
    tool_id: &str,
    _session_id: String,
    _db_pool: Arc<SqlitePool>,
    _session_manager: Arc<SessionManager>,
) -> Result<Option<Box<dyn BuiltinMCPServer>>, String> {
    match tool_id {
        "bootstrap" => Ok(Some(Box::new(
            crate::mcp::builtin::bootstrap::BootstrapServer::new(),
        ))),
        "knowledge" => Ok(Some(Box::new(
            crate::mcp::builtin::knowledge::KnowledgeServer::new(_session_id, _db_pool).await?,
        ))),
        "planning" => Ok(Some(Box::new(
            crate::mcp::builtin::planning::PlanningServer::new(_session_id, _db_pool).await?,
        ))),
        "playbook" => Ok(Some(Box::new(
            crate::mcp::builtin::playbook::PlaybookServer::new(_session_id, _db_pool).await?,
        ))),
        "assistant" => Ok(Some(Box::new(
            crate::mcp::builtin::assistant::AssistantServer::new(_db_pool).await?,
        ))),
        "workspace" => Ok(Some(Box::new(
            crate::mcp::builtin::workspace::WorkspaceServer::new(_session_manager),
        ))),
        "content_store" | "contentstore" => Ok(Some(Box::new(
            crate::mcp::builtin::content_store::ContentStoreServer::new(_session_manager),
        ))),
        "ui" => Ok(Some(Box::new(crate::mcp::builtin::ui::UiServer::new()))),
        _ => Ok(None), // Unknown tool, skip
    }
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
