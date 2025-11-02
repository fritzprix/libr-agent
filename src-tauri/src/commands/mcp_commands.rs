/// MCP (Model Context Protocol) server management commands
///
/// This module contains all commands related to managing external and built-in MCP servers,
/// including server lifecycle, tool listing, and tool execution.
use crate::mcp::types::{
    MCPServerConfigV2, MCPServerConfigWrapper, OAuthConfig, ServiceContext, ServiceContextOptions,
};
use crate::mcp::{MCPResponse, MCPServerConfig, MCPServerManager, MCPTool};
use crate::state::get_mcp_manager;
use std::collections::HashMap;

// ============================================================================
// External MCP Server Commands
// ============================================================================

/// Starts an external MCP server process.
#[tauri::command]
pub async fn start_mcp_server(config: MCPServerConfig) -> Result<String, String> {
    get_mcp_manager()
        .start_server(config)
        .await
        .map_err(|e| e.to_string())
}

/// Stops a running external MCP server.
#[tauri::command]
pub async fn stop_mcp_server(server_name: String) -> Result<(), String> {
    get_mcp_manager()
        .stop_server(&server_name)
        .await
        .map_err(|e| e.to_string())
}

/// Calls a tool on an external MCP server.
#[tauri::command]
pub async fn call_mcp_tool(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> MCPResponse {
    get_mcp_manager()
        .call_tool(&server_name, &tool_name, arguments)
        .await
}

/// Performs text generation on an external MCP server.
#[tauri::command]
pub async fn sample_from_mcp_server(
    server_name: String,
    prompt: String,
    options: Option<serde_json::Value>,
) -> Result<MCPResponse, String> {
    let sampling_options = if let Some(opts) = options {
        Some(
            serde_json::from_value::<crate::mcp::SamplingOptions>(opts)
                .map_err(|e| format!("Invalid sampling options: {e}"))?,
        )
    } else {
        None
    };

    let request = crate::mcp::SamplingRequest {
        prompt,
        options: sampling_options,
    };

    Ok(get_mcp_manager()
        .sample_from_model(&server_name, request)
        .await)
}

/// Lists the tools available on a specific external MCP server.
#[tauri::command]
pub async fn list_mcp_tools(server_name: String) -> Result<Vec<MCPTool>, String> {
    get_mcp_manager()
        .list_tools(&server_name)
        .await
        .map_err(|e| e.to_string())
}

/// Starts servers from a dynamic configuration object and lists their available tools.
///
/// This command now supports both V1 (legacy) and V2 (MCP 2025-06-18 spec) configurations.
/// It automatically detects the format and converts legacy configs to V2.
///
/// # Arguments
/// * `config` - A `serde_json::Value` containing the server configurations in either format.
///
/// # Returns
/// A `Result` containing a `HashMap` where keys are server names and values are vectors
/// of `MCPTool` objects. Returns an error string if the configuration is invalid.
#[tauri::command]
pub async fn list_tools_from_config(
    config: serde_json::Value,
) -> Result<HashMap<String, Vec<MCPTool>>, String> {
    println!("🚀 [TAURI] list_tools_from_config called!");
    println!(
        "🚀 [TAURI] Config received: {}",
        serde_json::to_string_pretty(&config).unwrap_or_default()
    );

    // Parse server configurations with automatic V1/V2 detection
    let servers_config =
        if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
            // MCPConfig format: Convert mcpServers object to V2 configs
            println!("🚀 [TAURI] Processing mcpServers format");
            let mut server_list = Vec::new();

            for (name, server_config) in mcp_servers.iter() {
                let mut server_value = server_config.clone();
                // Add the name field
                if let serde_json::Value::Object(ref mut obj) = server_value {
                    obj.insert("name".to_string(), serde_json::Value::String(name.clone()));
                }

                // Try parsing as V2/V1 wrapper (auto-detects format)
                let wrapper: MCPServerConfigWrapper = serde_json::from_value(server_value)
                    .map_err(|e| format!("Invalid server config for '{name}': {e}"))?;
                let v2_config: MCPServerConfigV2 = wrapper.into();
                server_list.push(v2_config);
            }
            server_list
        } else if let Some(servers_array) = config.get("servers").and_then(|v| v.as_array()) {
            // Legacy format: servers array
            println!("🚀 [TAURI] Processing legacy servers array format");
            let mut server_list = Vec::new();
            for server_value in servers_array {
                let wrapper: MCPServerConfigWrapper = serde_json::from_value(server_value.clone())
                    .map_err(|e| format!("Invalid server config: {e}"))?;
                let v2_config: MCPServerConfigV2 = wrapper.into();
                server_list.push(v2_config);
            }
            server_list
        } else {
            return Err("Invalid config: missing mcpServers object or servers array".to_string());
        };

    println!(
        "🚀 [TAURI] Found {} servers in config",
        servers_config.len()
    );

    let manager = get_mcp_manager();

    let mut tools_by_server: HashMap<String, Vec<MCPTool>> = HashMap::new();

    // Start servers from config and collect their tools
    for server_cfg in servers_config {
        let server_name = server_cfg.name.clone();
        if !manager.is_server_alive(&server_name).await {
            println!("🚀 [TAURI] Starting server: {server_name}");
            // Use native V2 config support to preserve OAuth, HTTP headers, and security settings
            if let Err(e) = manager.start_server_v2(server_cfg).await {
                eprintln!("❌ [TAURI] Failed to start server {server_name}: {e}");
                // Insert empty tools array for failed server
                tools_by_server.insert(server_name, Vec::new());
                continue; // Skip to the next server if this one fails to start
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        } else {
            println!("🚀 [TAURI] Server {server_name} already running");
        }

        // Fetch tools for the server we just ensured is running
        match manager.list_tools(&server_name).await {
            Ok(tools) => {
                println!(
                    "✅ [TAURI] Found {} tools for server '{}'",
                    tools.len(),
                    server_name
                );
                tools_by_server.insert(server_name, tools);
            }
            Err(e) => {
                eprintln!("❌ [TAURI] Error listing tools for '{server_name}': {e}");
                // Insert empty tools array for failed server
                tools_by_server.insert(server_name, Vec::new());
            }
        }
    }

    let total_tools: usize = tools_by_server.values().map(|tools| tools.len()).sum();
    println!(
        "✅ [TAURI] Total tools collected: {} across {} servers",
        total_tools,
        tools_by_server.len()
    );
    Ok(tools_by_server)
}

/// Returns a list of names for all currently connected external MCP servers.
#[tauri::command]
pub async fn get_connected_servers() -> Vec<String> {
    get_mcp_manager().get_connected_servers().await
}

/// Checks if a specific external MCP server is currently alive and responsive.
#[tauri::command]
pub async fn check_server_status(server_name: String) -> bool {
    get_mcp_manager().is_server_alive(&server_name).await
}

/// Checks the status of all managed external MCP servers.
///
/// # Returns
/// A `HashMap` where keys are server names and values are booleans indicating if the
/// server is alive.
#[tauri::command]
pub async fn check_all_servers_status() -> HashMap<String, bool> {
    get_mcp_manager().check_all_servers().await
}

/// Lists all available tools from all connected external MCP servers.
#[tauri::command]
pub async fn list_all_tools() -> Result<Vec<MCPTool>, String> {
    get_mcp_manager()
        .list_all_tools()
        .await
        .map_err(|e| e.to_string())
}

/// Retrieves the list of validated tools for a specific external server.
#[tauri::command]
pub async fn get_validated_tools(server_name: String) -> Result<Vec<MCPTool>, String> {
    get_mcp_manager()
        .get_validated_tools(&server_name)
        .await
        .map_err(|e| e.to_string())
}

/// Validates the JSON schema of a single MCP tool.
#[tauri::command]
pub fn validate_tool_schema(tool: MCPTool) -> Result<(), String> {
    MCPServerManager::validate_tool_schema(&tool).map_err(|e| e.to_string())
}

// ============================================================================
// Built-in MCP Server Commands
// ============================================================================

/// Lists the names of all available built-in MCP servers.
#[tauri::command]
pub async fn list_builtin_servers() -> Vec<String> {
    get_mcp_manager().list_builtin_servers().await
}

/// Lists all tools available from the built-in MCP servers.
///
/// # Arguments
/// * `server_name` - An optional string. If provided, lists tools only for that
///   specific built-in server. Otherwise, lists tools from all built-in servers.
#[tauri::command]
pub async fn list_builtin_tools(server_name: Option<String>) -> Vec<MCPTool> {
    match server_name {
        Some(name) => get_mcp_manager().list_builtin_tools_for(&name).await,
        None => get_mcp_manager().list_builtin_tools().await,
    }
}

/// Calls a tool on one of the built-in MCP servers.
#[tauri::command]
pub async fn call_builtin_tool(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> MCPResponse {
    get_mcp_manager()
        .call_builtin_tool(&server_name, &tool_name, arguments)
        .await
}

// ============================================================================
// Unified MCP Commands (Built-in + External)
// ============================================================================

/// Lists all tools from both built-in and external MCP servers in a unified list.
#[tauri::command]
pub async fn list_all_tools_unified() -> Result<Vec<MCPTool>, String> {
    get_mcp_manager()
        .list_all_tools_unified()
        .await
        .map_err(|e| e.to_string())
}

/// Calls a tool on either a built-in or external MCP server, determined by the server name.
#[tauri::command]
pub async fn call_tool_unified(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> MCPResponse {
    get_mcp_manager()
        .call_tool_unified(&server_name, &tool_name, arguments)
        .await
}

// ============================================================================
// Service Context Commands
// ============================================================================

/// Retrieves the service context for a given MCP server.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server.
/// * `options` - Optional context options for the service.
///
/// # Returns
/// A `Result` containing the service context on success, or an error string on failure.
#[tauri::command]
pub async fn get_service_context(
    server_id: String,
    options: Option<ServiceContextOptions>,
) -> Result<ServiceContext, String> {
    get_mcp_manager()
        .get_service_context(&server_id, options)
        .await
}

/// Switches the context for a given MCP server.
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server.
/// * `options` - The context options to switch to.
///
/// # Returns
/// A `Result` indicating success or an error string on failure.
#[tauri::command]
#[allow(dead_code)]
pub async fn switch_context(
    server_id: String,
    options: ServiceContextOptions,
) -> Result<(), String> {
    get_mcp_manager().switch_context(&server_id, options).await
}

// ============================================================================
// OAuth 2.1 Authentication Commands
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
/// ```
/// let (url, state) = start_oauth_flow(
///     "github-mcp".to_string(),
///     oauth_config
/// ).await?;
/// // Open URL in browser: open::that(url)?;
/// ```
#[tauri::command]
pub async fn start_oauth_flow(
    server_id: String,
    config: OAuthConfig,
) -> Result<(String, String), String> {
    log::info!("Starting OAuth flow for server: {server_id}");

    let oauth_manager = get_mcp_manager().get_oauth_manager().await;
    oauth_manager
        .start_authorization_flow(&config, &server_id)
        .await
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
    server_id: String,
    config: OAuthConfig,
    authorization_code: String,
    state: String,
) -> Result<String, String> {
    log::info!("Completing OAuth flow for server: {server_id}");

    let oauth_manager = get_mcp_manager().get_oauth_manager().await;

    // Exchange code for token
    let access_token = oauth_manager
        .exchange_code_for_token(&config, &server_id, &authorization_code, &state)
        .await?;

    // Store token securely in OS keychain
    crate::mcp::keychain::store_token_securely(&server_id, &access_token).await?;

    log::info!("Successfully completed OAuth flow for server: {server_id}");
    Ok(format!(
        "OAuth flow completed successfully for server: {server_id}"
    ))
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
