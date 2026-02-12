/// MCP (Model Context Protocol) server management commands
///
/// This module contains all commands related to managing built-in MCP servers,
/// tool listing, and tool execution. Session-isolated external servers are
/// managed through the MCPServiceProxyManager per session.
use crate::mcp::types::{
    BuiltinServerInfo, MCPServerConfig, OAuthConfig, ServiceContext, ServiceContextOptions,
};
use crate::mcp::{MCPResponse, MCPServerManager, MCPTool};
use std::collections::HashMap;

// ============================================================================
// Deprecated External MCP Server Commands (Legacy Support Only)
// ============================================================================
//
// These commands are deprecated in favor of session-isolated server management
// via MCPServiceProxyManager. They are kept for backward compatibility but should
// not be used for new functionality.

/// Starts an external MCP server process.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture. External servers are now managed per-session
/// through MCPServiceProxyManager.
#[tauri::command]
pub async fn start_mcp_server(_config: MCPServerConfig) -> Result<String, String> {
    log::warn!("start_mcp_server: Using deprecated global MCP manager. Use session-isolated servers instead.");
    Err(
        "Global MCP server management is deprecated. Use session-isolated server configuration."
            .to_string(),
    )
}

/// Stops a running external MCP server.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
#[tauri::command]
pub async fn stop_mcp_server(server_name: String) -> Result<(), String> {
    log::warn!(
        "stop_mcp_server: Using deprecated global MCP manager. Server: {}",
        server_name
    );
    Err(
        "Global MCP server management is deprecated. Use session-isolated server configuration."
            .to_string(),
    )
}

/// Calls a tool on an external MCP server.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
#[tauri::command]
pub async fn call_mcp_tool(
    server_name: String,
    tool_name: String,
    _arguments: serde_json::Value,
    _request_id: Option<String>,
) -> MCPResponse {
    log::warn!(
        "call_mcp_tool: Using deprecated global MCP manager. Server: {}, Tool: {}",
        server_name,
        tool_name
    );
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id: None,
        result: None,
        error: Some(crate::mcp::types::MCPError {
            code: -32603,
            message: "Global MCP server management is deprecated. Use session-isolated server configuration.".to_string(),
            data: None,
        }),
    }
}

/// Performs text generation on an external MCP server.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
#[tauri::command]
pub async fn sample_from_mcp_server(
    _server_name: String,
    _prompt: String,
    _options: Option<serde_json::Value>,
    _request_id: Option<String>,
) -> Result<MCPResponse, String> {
    log::warn!("sample_from_mcp_server: Using deprecated global MCP manager.");
    Err(
        "Global MCP server management is deprecated. Use session-isolated server configuration."
            .to_string(),
    )
}

/// Lists the tools available on a specific external MCP server.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
#[tauri::command]
pub async fn list_mcp_tools(server_name: String) -> Result<Vec<MCPTool>, String> {
    log::warn!(
        "list_mcp_tools: Using deprecated global MCP manager. Server: {}",
        server_name
    );
    Err(
        "Global MCP server management is deprecated. Use session-isolated server configuration."
            .to_string(),
    )
}

/// Starts servers from a dynamic configuration object and lists their available tools.
///
/// This command now supports both V1 (legacy) and V2 (MCP 2025-06-18 spec) configurations.
/// It automatically detects the format and converts legacy configs to V2.
///
/// list_tools_from_config REMOVED (Deprecated)
///
/// This command was used to eager-load all servers and list tools globally.
/// It has been removed in favor of session-isolated tool discovery.
#[tauri::command]
pub async fn list_tools_from_config(
    _config: serde_json::Value,
) -> Result<HashMap<String, Vec<MCPTool>>, String> {
    log::warn!(
        "list_tools_from_config called (DEPRECATED). This command is incompatible with session-isolated MCP servers."
    );

    Err(
        "list_tools_from_config is deprecated and no longer supported.\n\n\
This command previously started servers globally and returned a merged tool list, but LibrAgent now enforces per-session MCP isolation.\n\n\
Recovery:\n\
- Configure external MCP servers on the agent/session (Agent V2) instead of using global discovery\n\
- Use session-scoped tool discovery via the Agent V2 workflow (tools are collected from MCPServiceProxy per session)\n\
\n\
If you're calling this from legacy frontend code, migrate to the Agent V2 session tool listing flow."
            .to_string(),
    )
}

/// Returns a list of names for all currently connected external MCP servers.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
#[tauri::command]
pub async fn get_connected_servers() -> Result<Vec<String>, String> {
    log::warn!("get_connected_servers: Using deprecated global MCP manager.");
    Err(
        "get_connected_servers is deprecated and incompatible with session-isolated MCP servers.\n\n\
Recovery:\n\
- Use Agent V2 session tool discovery instead (tools are collected from MCPServiceProxy per session)\n\
- If you need to check external server connectivity, do it within the session-scoped MCP managers"
            .to_string(),
    )
}

/// Checks if a specific external MCP server is currently alive and responsive.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
#[tauri::command]
pub async fn check_server_status(server_name: String) -> Result<bool, String> {
    log::warn!(
        "check_server_status: Using deprecated global MCP manager. Server: {}",
        server_name
    );
    Err(
        "check_server_status is deprecated and incompatible with session-isolated MCP servers.\n\n\
Recovery:\n\
- Manage external servers per-session via Agent V2 configuration\n\
- Use session-scoped tool calls (through MCPServiceProxy) to validate connectivity"
            .to_string(),
    )
}

/// Checks the status of all managed external MCP servers.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
///
/// # Returns
/// A `HashMap` where keys are server names and values are booleans indicating if the
/// server is alive.
#[tauri::command]
pub async fn check_all_servers_status() -> Result<HashMap<String, bool>, String> {
    log::warn!("check_all_servers_status: Using deprecated global MCP manager.");
    Err(
        "check_all_servers_status is deprecated and incompatible with session-isolated MCP servers.\n\n\
Recovery:\n\
- Manage external servers per-session via Agent V2 configuration\n\
- Use session-scoped tool discovery and tool calls (through MCPServiceProxy) instead of global polling"
            .to_string(),
    )
}

/// Lists all available tools from all connected external MCP servers.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
#[tauri::command]
pub async fn list_all_tools() -> Result<Vec<MCPTool>, String> {
    log::warn!("list_all_tools: Using deprecated global MCP manager.");
    Err(
        "Global MCP server management is deprecated. Use session-isolated server configuration."
            .to_string(),
    )
}

/// Retrieves the list of validated tools for a specific external server.
///
/// ⚠️ DEPRECATED: This uses the global MCPServerManager which is incompatible with
/// Session Isolation architecture.
#[tauri::command]
pub async fn get_validated_tools(server_name: String) -> Result<Vec<MCPTool>, String> {
    log::warn!(
        "get_validated_tools: Using deprecated global MCP manager. Server: {}",
        server_name
    );
    Err(
        "Global MCP server management is deprecated. Use session-isolated server configuration."
            .to_string(),
    )
}

/// Validates the JSON schema of a single MCP tool.
///
/// This is a utility function that doesn't depend on global state.
#[tauri::command]
pub fn validate_tool_schema(tool: MCPTool) -> Result<(), String> {
    MCPServerManager::validate_tool_schema(&tool).map_err(|e| e.to_string())
}

// ============================================================================
// Built-in MCP Server Commands (Session-Agnostic)
// ============================================================================

/// Lists the names of all available built-in MCP servers.
///
/// Built-in servers are available to all sessions (workspace, planning, knowledge, etc.)
/// This returns static definitions, not per-session instances.
#[tauri::command]
pub async fn list_builtin_servers() -> Vec<String> {
    MCPServerManager::list_available_builtin_server_definitions()
        .into_iter()
        .map(|info| info.name)
        .collect()
}

/// Lists all tools available from the built-in MCP servers.
///
/// # Arguments
/// * `server_name` - An optional string. If provided, lists tools only for that
///   specific built-in server. Otherwise, lists tools from all built-in servers.
///
/// Note: This returns the static schema, not session-specific instances.
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
        .flat_map(|_info| {
            // Return empty tools for now - actual tools come from session-specific proxies
            Vec::new()
        })
        .collect()
}

/// Lists all built-in MCP servers with their metadata.
///
/// Returns a vector of `BuiltinServerInfo` containing:
/// - Server name (e.g., "workspace", "contentstore")
/// - UI metadata (displayName, description, category, icon)
/// - Tool count
///
/// This enables the frontend to dynamically discover servers and display
/// proper metadata without hardcoding.
#[tauri::command]
pub async fn list_builtin_servers_with_metadata() -> Vec<BuiltinServerInfo> {
    MCPServerManager::list_available_builtin_server_definitions()
}

/// Lists all POSSIBLE builtin server definitions for UI configuration
/// This shows what builtin tools are available to configure for assistants/agents,
/// not what's currently instantiated in the global registry.
#[tauri::command]
pub fn list_available_builtin_server_definitions() -> Vec<BuiltinServerInfo> {
    MCPServerManager::list_available_builtin_server_definitions()
}

/// Calls a tool on one of the built-in MCP servers.
///
/// ⚠️ DEPRECATED: Built-in tools are now managed per-session through MCPServiceProxyManager.
/// This global version is kept for backward compatibility only.
#[tauri::command]
pub async fn call_builtin_tool(
    _server_name: String,
    _tool_name: String,
    _arguments: serde_json::Value,
    _request_id: Option<String>,
) -> MCPResponse {
    log::warn!("call_builtin_tool: Global version deprecated. Use session-specific proxy instead.");
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id: None,
        result: None,
        error: Some(crate::mcp::types::MCPError {
            code: -32603,
            message: "Global built-in tool execution is deprecated. Use session-specific proxies."
                .to_string(),
            data: None,
        }),
    }
}

// ============================================================================
// Unified MCP Commands (Built-in + External)
// ============================================================================

/// Lists all tools from both built-in and external MCP servers in a unified list.
///
/// ⚠️ DEPRECATED: Use session-isolated tool discovery instead.
#[tauri::command]
pub async fn list_all_tools_unified() -> Result<Vec<MCPTool>, String> {
    log::warn!("list_all_tools_unified: Using deprecated global MCP manager.");
    Err(
        "Global MCP server management is deprecated. Use session-isolated server configuration."
            .to_string(),
    )
}

/// Calls a tool on either a built-in or external MCP server, determined by the server name.
///
/// ⚠️ DEPRECATED: Use session-specific tool execution through MCPServiceProxyManager.
#[tauri::command]
#[allow(dead_code)]
pub async fn call_tool_unified(
    _server_name: String,
    _tool_name: String,
    _arguments: serde_json::Value,
    _request_id: Option<String>,
) -> MCPResponse {
    log::warn!("call_tool_unified: Using deprecated global MCP manager.");
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id: None,
        result: None,
        error: Some(crate::mcp::types::MCPError {
            code: -32603,
            message: "Global tool execution is deprecated. Use session-specific proxies."
                .to_string(),
            data: None,
        }),
    }
}

// ============================================================================
// Service Context Commands (Session-Agnostic)
// ============================================================================

/// Retrieves the service context for a given MCP server.
///
/// ⚠️ NOTE: Service contexts are now retrieved per-session from MCPServiceProxy.
/// This global version may not reflect session-specific state.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server.
/// * `options` - Optional context options for the service.
///
/// # Returns
/// A `Result` containing the service context on success, or an error string on failure.
#[tauri::command]
pub async fn get_service_context(
    _server_id: String,
    _options: Option<ServiceContextOptions>,
) -> Result<ServiceContext, String> {
    log::warn!("get_service_context: Global version does not provide session-specific state. Use session proxies instead.");
    Err("Use session-specific MCPServiceProxy for accurate service context.".to_string())
}

// ============================================================================
// OAuth 2.1 Authentication Commands (Utility Functions)
// ============================================================================

/// Starts an OAuth 2.1 authorization flow with PKCE for an MCP server.
///
/// This command initiates the OAuth flow by:
/// 1. Discovering OAuth endpoints (if discovery URL is provided)
/// 2. Creating a PKCE challenge
/// 3. Generating an authorization URL
/// 4. Storing PKCE verifier and CSRF token for later validation
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
/// * `config` - OAuth configuration containing client_id, endpoints, scopes, etc.
///
/// # Returns
/// A tuple containing:
/// - `authorization_url`: The URL to open in the user's browser
/// - `state`: The CSRF state token for validation
///
/// # Example
/// ```ignore
/// let (url, state) = start_oauth_flow(
///     "github-mcp".to_string(),
///     oauth_config
/// ).await?;
/// // Open URL in browser: open::that(url)?;
/// ```
#[tauri::command]
pub async fn start_oauth_flow(
    _server_id: String,
    _config: OAuthConfig,
) -> Result<(String, String), String> {
    log::warn!("start_oauth_flow: Global OAuth management is deprecated.");
    Err(
        "Global OAuth management is not available. Configure OAuth at server creation time."
            .to_string(),
    )
}

/// Completes an OAuth 2.1 authorization flow by exchanging the authorization code for an access token.
///
/// This command:
/// 1. Validates the CSRF state token
/// 2. Retrieves the stored PKCE verifier
/// 3. Exchanges the authorization code for an access token
/// 4. Stores the token securely in the OS keychain
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
/// * `config` - OAuth configuration used for the flow
/// * `authorization_code` - The code received from the OAuth callback
/// * `state` - The CSRF state token for validation
///
/// # Returns
/// Success message if token was stored successfully
///
/// # Security
/// - Validates CSRF token to prevent CSRF attacks
/// - Uses PKCE to prevent authorization code interception
/// - Stores token in OS keychain (never in plain text)
#[tauri::command]
pub async fn complete_oauth_flow(
    _server_id: String,
    _config: OAuthConfig,
    _authorization_code: String,
    _state: String,
) -> Result<String, String> {
    log::warn!("complete_oauth_flow: Global OAuth management is deprecated.");
    Err(
        "Global OAuth management is not available. Configure OAuth at server creation time."
            .to_string(),
    )
}

/// Checks if an OAuth token exists in the OS keychain for a given server.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// `true` if a token exists, `false` otherwise
#[tauri::command]
pub async fn has_oauth_token(server_id: String) -> bool {
    crate::mcp::keychain::has_token(&server_id).await
}

/// Retrieves a cached OAuth token from the OS keychain.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// `Some(token)` if found, `None` if not found
///
/// # Security
/// This command should be used carefully. Consider whether the frontend
/// actually needs the raw token or just needs to know if it exists.
#[tauri::command]
pub async fn get_oauth_token(server_id: String) -> Result<Option<String>, String> {
    crate::mcp::keychain::get_cached_token(&server_id).await
}

/// Revokes and deletes an OAuth token from the OS keychain.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// Success message if token was deleted
#[tauri::command]
pub async fn revoke_oauth_token(server_id: String) -> Result<String, String> {
    crate::mcp::keychain::delete_token(&server_id).await?;
    Ok(format!(
        "OAuth token revoked successfully for server: {server_id}"
    ))
}
