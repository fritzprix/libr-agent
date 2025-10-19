/// MCP (Model Context Protocol) server management commands
///
/// This module contains all commands related to managing external and built-in MCP servers,
/// including server lifecycle, tool listing, and tool execution.
use crate::mcp::types::{ServiceContext, ServiceContextOptions};
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
/// This command supports two configuration formats: a "Claude format" with an `mcpServers`
/// object and a legacy format with a `servers` array. It will start any servers from
/// the config that are not already running, then queries each one for its list of tools.
///
/// # Arguments
/// * `config` - A `serde_json::Value` containing the server configurations.
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

    // Support for Claude format: handle mcpServers object or servers array
    let servers_config =
        if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
            // Claude format: Convert mcpServers object to an array of MCPServerConfig
            println!("🚀 [TAURI] Processing Claude format (mcpServers)");
            let mut server_list = Vec::new();

            for (name, server_config) in mcp_servers.iter() {
                let mut server_value = server_config.clone();
                // Add the name field
                if let serde_json::Value::Object(ref mut obj) = server_value {
                    obj.insert("name".to_string(), serde_json::Value::String(name.clone()));
                    obj.insert(
                        "transport".to_string(),
                        serde_json::Value::String("stdio".to_string()),
                    );
                }
                let server_cfg: MCPServerConfig = serde_json::from_value(server_value)
                    .map_err(|e| format!("Invalid server config: {e}"))?;
                server_list.push(server_cfg);
            }
            server_list
        } else if let Some(servers_array) = config.get("servers").and_then(|v| v.as_array()) {
            // Legacy format: servers array
            println!("🚀 [TAURI] Processing legacy format (servers array)");
            let mut server_list = Vec::new();
            for server_value in servers_array {
                let server_cfg: MCPServerConfig = serde_json::from_value(server_value.clone())
                    .map_err(|e| format!("Invalid server config: {e}"))?;
                server_list.push(server_cfg);
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
            if let Err(e) = manager.start_server(server_cfg).await {
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
