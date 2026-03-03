/// MCP (Model Context Protocol) server management commands
///
/// This module contains commands related to built-in MCP servers,
/// server probing, OAuth token management, and tool schema validation.
/// Session-isolated external servers are managed through MCPServiceProxyManager per session.
use crate::mcp::types::BuiltinServerInfo;
use crate::mcp::{MCPServerManager, MCPTool};

/// Probe a single MCP server by ID: connect, list tools, disconnect.
///
/// Lightweight alternative to the deprecated `list_tools_from_config`. No agent session
/// is created. The server process is spawned (stdio) or connected (HTTP), tools are
/// fetched, and the connection is torn down — all inside this single call.
///
/// Returns the number of tools discovered and their names.
#[tauri::command]
pub async fn probe_mcp_server(server_id: String) -> Result<Vec<MCPTool>, String> {
    crate::services::mcp_server_service::McpServerService::probe_server(&server_id).await
}

/// Validates the JSON schema of a single MCP tool.
///
/// Stateless utility — no global state involved.
#[tauri::command]
pub fn validate_tool_schema(tool: MCPTool) -> Result<(), String> {
    MCPServerManager::validate_tool_schema(&tool).map_err(|e| e.to_string())
}

// ============================================================================
// Built-in MCP Server Commands (Session-Agnostic)
// ============================================================================

/// Lists the names of all available built-in MCP servers.
#[tauri::command]
pub async fn list_builtin_servers() -> Vec<String> {
    MCPServerManager::list_available_builtin_server_definitions()
        .into_iter()
        .map(|info| info.name)
        .collect()
}

/// Lists all tools available from the built-in MCP servers.
///
/// `server_name` filters to a specific server; omit for all servers.
/// Returns static schemas — actual tools come from session-specific proxies.
#[tauri::command]
pub async fn list_builtin_tools(server_name: Option<String>) -> Vec<MCPTool> {
    MCPServerManager::list_available_builtin_server_definitions()
        .into_iter()
        .filter(|info| {
            server_name.is_none()
                || server_name
                    .as_ref()
                    .map(|n| n == &info.name)
                    .unwrap_or(false)
        })
        .flat_map(|_info| Vec::<MCPTool>::new())
        .collect()
}

/// Lists all built-in MCP servers with their UI metadata (displayName, description, icon, etc.).
#[tauri::command]
pub async fn list_builtin_servers_with_metadata() -> Vec<BuiltinServerInfo> {
    MCPServerManager::list_available_builtin_server_definitions()
}

/// Lists all possible builtin server definitions for UI configuration.
/// Shows what builtin tools are available to assign to assistants/agents.
#[tauri::command]
pub fn list_available_builtin_server_definitions() -> Vec<BuiltinServerInfo> {
    MCPServerManager::list_available_builtin_server_definitions()
}

// ============================================================================
// OAuth Keychain Commands
// ============================================================================

/// Returns `true` if an OAuth token exists in the OS keychain for the given server.
#[tauri::command]
pub async fn has_oauth_token(server_id: String) -> bool {
    crate::mcp::keychain::has_token(&server_id).await
}

/// Retrieves a cached OAuth token from the OS keychain.
#[tauri::command]
pub async fn get_oauth_token(server_id: String) -> Result<Option<String>, String> {
    crate::mcp::keychain::get_cached_token(&server_id).await
}

/// Revokes and deletes an OAuth token from the OS keychain.
#[tauri::command]
pub async fn revoke_oauth_token(server_id: String) -> Result<String, String> {
    crate::mcp::keychain::delete_token(&server_id).await?;
    Ok(format!(
        "OAuth token revoked successfully for server: {server_id}"
    ))
}
