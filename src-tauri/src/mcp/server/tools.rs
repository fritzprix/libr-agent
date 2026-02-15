use super::MCPServerManager;
use crate::mcp::schema::JSONSchemaType;
use crate::mcp::types::{
    BuiltinServerInfo, JsonRpcId, MCPError, MCPResponse, MCPTool, SamplingRequest, ServiceContext,
    ServiceContextOptions, TransportConfig,
};
use anyhow::Result;
use log::{debug, error, info, warn};
use rmcp::model::CallToolRequestParam;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// Helper function to convert serde_json::Value to JsonRpcId
fn value_to_json_rpc_id(value: serde_json::Value) -> JsonRpcId {
    match value {
        serde_json::Value::String(s) => JsonRpcId::String(s),
        serde_json::Value::Number(n) => JsonRpcId::Number(n.as_i64().unwrap_or(0)),
        serde_json::Value::Null => JsonRpcId::Null,
        // Fallback: convert any other type to string
        _ => JsonRpcId::String(value.to_string()),
    }
}

pub async fn sample_from_model(
    manager: &MCPServerManager,
    server_name: &str,
    request: SamplingRequest,
    request_id: Option<serde_json::Value>,
) -> MCPResponse {
    let connections = manager.connections.lock().await;
    let request_id = value_to_json_rpc_id(
        request_id.unwrap_or_else(|| serde_json::Value::String(Uuid::new_v4().to_string())),
    );

    if let Some(_connection) = connections.get(server_name) {
        // This needs to be implemented once RMCP supports sampling.
        // For now, return a temporary error.
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(request_id),
            result: None,
            error: Some(MCPError {
                code: -32601,
                message: "Sampling not yet implemented in RMCP".to_string(),
                data: Some(serde_json::json!({
                    "server_name": server_name,
                    "request": request
                })),
            }),
        }
    } else {
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(request_id),
            result: None,
            error: Some(MCPError {
                code: -32002,
                message: format!("Server '{server_name}' not found"),
                data: None,
            }),
        }
    }
}

pub async fn call_tool(
    manager: &MCPServerManager,
    server_name: &str,
    tool_name: &str,
    arguments: serde_json::Value,
    request_id: Option<serde_json::Value>,
) -> MCPResponse {
    let connections = manager.connections.lock().await;

    // Use provided request_id or generate a new unique ID, then convert to JsonRpcId
    let request_id = value_to_json_rpc_id(
        request_id.unwrap_or_else(|| serde_json::Value::String(Uuid::new_v4().to_string())),
    );

    if let Some(connection) = connections.get(server_name) {
        // Use the rmcp API - CallToolRequestParam struct
        let args_map = if let serde_json::Value::Object(obj) = arguments {
            obj
        } else {
            serde_json::Map::new()
        };

        let call_param = CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments: Some(args_map),
        };

        match connection.client.call_tool(call_param).await {
            Ok(result) => {
                // Log the raw rmcp response first (before serialization)
                info!("Raw rmcp CallToolResult (before serialization): {result:?}");

                // Handle the rmcp CallToolResult more carefully
                let result_value = match serde_json::to_value(&result) {
                    Ok(value) => value,
                    Err(e) => {
                        error!("Failed to serialize tool result: {e}");
                        return MCPResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(request_id.clone()),
                            result: None,
                            error: Some(MCPError {
                                code: -32603,
                                message: format!("Failed to serialize result: {e}"),
                                data: None,
                            }),
                        };
                    }
                };

                // Debug log to check the original structure
                info!("Original rmcp result: {result:?}");
                info!("Serialized result: {result_value}");

                // Detect and add logging for UI resources
                if let Some(content) = result_value.get("content") {
                    if let Some(content_array) = content.as_array() {
                        for (i, item) in content_array.iter().enumerate() {
                            if item.get("type").and_then(|t| t.as_str()) == Some("resource") {
                                debug!("Found UI resource at index {i}: {item}");
                                if let Some(resource) = item.get("resource") {
                                    debug!("Resource details: {resource}");
                                    if resource.get("mimeType").is_none() {
                                        warn!("UI resource missing mimeType: {resource}");
                                    }
                                }
                            }
                        }
                    }
                }

                // Check if the result contains an error
                let contains_error = result_value.to_string().to_lowercase().contains("error");

                if contains_error
                    && result_value
                        .get("isError")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                {
                    // If isError is true, treat it as an error
                    let error_msg = result_value
                        .get("content")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|item| item.get("text"))
                        .and_then(|text| text.as_str())
                        .unwrap_or("Tool execution error");

                    MCPResponse::error(request_id, -32000, error_msg)
                } else {
                    // Normal response - preserve the rmcp structure as much as possible
                    MCPResponse {
                        jsonrpc: "2.0".to_string(),
                        id: Some(request_id),
                        result: Some(crate::mcp::types::MCPResponseResult::Generic(result_value)),
                        error: None,
                    }
                }
            }
            Err(e) => {
                error!("Error calling tool '{tool_name}': {e}");
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603, // Internal error
                        message: e.to_string(),
                        data: None,
                    }),
                }
            }
        }
    } else {
        error!("Server '{server_name}' not found");
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(request_id),
            result: None,
            error: Some(MCPError {
                code: -32601, // Method not found
                message: format!("Server '{server_name}' not found"),
                data: None,
            }),
        }
    }
}

pub async fn list_tools(manager: &MCPServerManager, server_name: &str) -> Result<Vec<MCPTool>> {
    let connections = manager.connections.lock().await;

    if let Some(connection) = connections.get(server_name) {
        debug!("Found connection for server: {server_name}");

        match connection.client.list_all_tools().await {
            Ok(tools_response) => {
                debug!("Raw tools response: {tools_response:?}");
                let mut tools = Vec::new();

                for tool in tools_response {
                    debug!("Processing tool: {tool:?}");

                    // Convert the input schema to our structured format
                    let input_schema_value = serde_json::to_value(tool.input_schema)
                        .unwrap_or_else(|e| {
                            warn!(
                                "Failed to serialize input_schema for tool {}: {}",
                                tool.name, e
                            );
                            serde_json::Value::Object(serde_json::Map::new())
                        });

                    let structured_schema =
                        crate::mcp::server_utils::convert_input_schema(input_schema_value);

                    let mcp_tool = MCPTool {
                        name: tool.name.to_string(),
                        title: None,
                        description: tool.description.unwrap_or_default().to_string(),
                        input_schema: structured_schema,
                        output_schema: None,
                        annotations: None,
                    };

                    debug!(
                        "Converted tool: {} with schema type: {:?}",
                        mcp_tool.name, mcp_tool.input_schema.schema_type
                    );
                    tools.push(mcp_tool);
                }

                debug!("Successfully converted {} tools", tools.len());
                Ok(tools)
            }
            Err(e) => {
                error!("Error listing tools: {e}");
                Err(anyhow::anyhow!("Failed to list tools: {e}"))
            }
        }
    } else {
        warn!("Server '{server_name}' not found in connections");
        Err(anyhow::anyhow!("Server '{server_name}' not found"))
    }
}

pub async fn list_all_tools(manager: &MCPServerManager) -> Result<Vec<MCPTool>> {
    let mut all_tools = Vec::new();
    let server_names: Vec<String> = {
        let connections = manager.connections.lock().await;
        connections
            .iter()
            .filter(|(_, conn)| matches!(conn.config.transport, TransportConfig::Http { .. }))
            .map(|(name, _)| name.clone())
            .collect()
    };

    for server_name in server_names {
        match list_tools(manager, &server_name).await {
            Ok(mut tools) => {
                // Prefix tool names with server name to avoid conflicts
                for tool in &mut tools {
                    tool.name = format!("{}__{}", server_name, tool.name);
                }
                all_tools.extend(tools);
            }
            Err(e) => {
                warn!("Failed to get tools from server {server_name}: {e}");
                // Continue with other servers instead of failing completely
            }
        }
    }

    Ok(all_tools)
}

pub async fn get_connected_servers(manager: &MCPServerManager) -> Vec<String> {
    let connections = manager.connections.lock().await;
    connections.keys().cloned().collect()
}

pub async fn is_server_alive(manager: &MCPServerManager, server_name: &str) -> bool {
    let connections = manager.connections.lock().await;
    connections.contains_key(server_name)
}

pub async fn check_all_servers(manager: &MCPServerManager) -> HashMap<String, bool> {
    let connections = manager.connections.lock().await;
    let mut status_map = HashMap::new();

    for server_name in connections.keys() {
        status_map.insert(server_name.to_string(), true);
    }

    status_map
}

pub fn validate_tool_schema(tool: &MCPTool) -> Result<()> {
    // Ensure the schema type is 'object'
    match &tool.input_schema.schema_type {
        JSONSchemaType::Object {
            properties,
            required,
            ..
        } => {
            // Validate required fields exist in properties
            if let (Some(required_fields), Some(props)) = (required, properties) {
                for req_field in required_fields {
                    if !props.contains_key(req_field) {
                        return Err(anyhow::anyhow!(
                            "Tool '{}' requires field '{}' but it's not defined in properties",
                            tool.name,
                            req_field
                        ));
                    }
                }
            } else if required.is_some() && properties.is_none() {
                return Err(anyhow::anyhow!(
                    "Tool '{}' has required fields but no properties defined",
                    tool.name
                ));
            }
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "Tool '{}' has invalid schema type, expected 'object'",
            tool.name
        )),
    }
}

pub async fn get_validated_tools(
    manager: &MCPServerManager,
    server_name: &str,
) -> Result<Vec<MCPTool>> {
    let tools = list_tools(manager, server_name).await?;
    let mut validated_tools = Vec::new();

    for tool in tools {
        match validate_tool_schema(&tool) {
            Ok(()) => {
                debug!("Tool '{}' passed validation", tool.name);
                validated_tools.push(tool);
            }
            Err(e) => {
                warn!("Tool '{}' failed validation: {}", tool.name, e);
                // Optionally, you could try to fix the schema or skip the tool
            }
        }
    }

    Ok(validated_tools)
}

pub async fn list_builtin_servers(manager: &MCPServerManager) -> Vec<String> {
    let servers = manager.builtin_servers.lock().await;
    match servers.as_ref() {
        Some(registry) => registry.list_servers(),
        None => Vec::new(),
    }
}

pub async fn list_builtin_tools(manager: &MCPServerManager) -> Vec<MCPTool> {
    let servers = manager.builtin_servers.lock().await;
    match servers.as_ref() {
        Some(registry) => registry.list_all_tools(),
        None => Vec::new(),
    }
}

pub async fn list_builtin_tools_for(manager: &MCPServerManager, server_name: &str) -> Vec<MCPTool> {
    // 1. Try to get tools from active registry first
    let servers = manager.builtin_servers.lock().await;
    let tools = match servers.as_ref() {
        Some(registry) => registry.list_tools_for_server(server_name),
        None => Vec::new(),
    };

    // 2. If no tools found (e.g. server is session-bound/virtual), try static definition
    if tools.is_empty() {
        get_static_tools_for_server(server_name)
    } else {
        tools
    }
}

pub async fn list_builtin_servers_with_metadata(
    manager: &MCPServerManager,
) -> Vec<BuiltinServerInfo> {
    let servers = manager.builtin_servers.lock().await;
    match servers.as_ref() {
        Some(registry) => registry
            .list_servers()
            .into_iter()
            .filter_map(|name| {
                registry.get_server(&name).map(|server| {
                    let tools = server.tools();
                    BuiltinServerInfo {
                        name: server.name().to_string(),
                        metadata: server.metadata(),
                        tool_count: tools.len(),
                    }
                })
            })
            .collect(),
        None => Vec::new(),
    }
}

/// Lists all POSSIBLE builtin server definitions (static metadata)
/// This is used by the UI to show all available builtin tools for configuration,
/// regardless of which servers are currently instantiated in the global registry.
/// Returns static metadata for all builtin servers that can be used in Agent V2 sessions.
pub fn list_available_builtin_server_definitions() -> Vec<BuiltinServerInfo> {
    use crate::mcp::builtin::BuiltinMCPServer;
    use crate::mcp::builtin::*;

    vec![
        BuiltinServerInfo {
            name: "bootstrap".to_string(),
            metadata: bootstrap::BootstrapServer::new().metadata(),
            tool_count: bootstrap::BootstrapServer::new().tools().len(),
        },
        BuiltinServerInfo {
            name: "knowledge".to_string(),
            metadata: knowledge::KnowledgeServer::metadata_static(),
            tool_count: knowledge::KnowledgeServer::tools_static().len(),
        },
        BuiltinServerInfo {
            name: "planning".to_string(),
            metadata: planning::PlanningServer::metadata_static(),
            tool_count: planning::PlanningServer::tools_static().len(),
        },
        BuiltinServerInfo {
            name: "playbook".to_string(),
            metadata: playbook::PlaybookServer::metadata_static(),
            tool_count: playbook::PlaybookServer::tools_static().len(),
        },
        BuiltinServerInfo {
            name: "assistant".to_string(),
            metadata: assistant::AssistantServer::metadata_static(),
            tool_count: assistant::AssistantServer::tools_static().len(),
        },
        BuiltinServerInfo {
            name: "workspace".to_string(),
            metadata: workspace::WorkspaceServer::metadata_static(),
            tool_count: workspace::WorkspaceServer::tools_static().len(),
        },
        BuiltinServerInfo {
            name: "contentstore".to_string(),
            metadata: content_store::ContentStoreServer::metadata_static(),
            tool_count: content_store::ContentStoreServer::tools_static().len(),
        },
        BuiltinServerInfo {
            name: "ui".to_string(),
            metadata: ui::UiServer::new().metadata(),
            tool_count: ui::UiServer::new().tools().len(),
        },
        BuiltinServerInfo {
            name: "browser".to_string(),
            metadata: browser::BrowserServer::metadata_static(),
            tool_count: browser::BrowserServer::tools_static().len(),
        },
        BuiltinServerInfo {
            name: "mcp_manager".to_string(),
            metadata: mcp_manager::MCPManagerServer::new().metadata(),
            tool_count: mcp_manager::MCPManagerServer::new().tools().len(),
        },
        BuiltinServerInfo {
            name: "session_api".to_string(),
            metadata: session_api::SessionApiServer::metadata_static(),
            tool_count: session_api::SessionApiServer::tools_static().len(),
        },
        BuiltinServerInfo {
            name: "skills".to_string(),
            metadata: skills::SkillsServer::metadata_static(),
            tool_count: skills::SkillsServer::tools_static().len(),
        },
    ]
}

pub async fn call_builtin_tool(
    manager: &MCPServerManager,
    server_name: &str,
    tool_name: &str,
    args: serde_json::Value,
    request_id: Option<serde_json::Value>,
) -> MCPResponse {
    debug!("call_builtin_tool: server_name='{server_name}', tool_name='{tool_name}', args={args}");

    let servers = manager.builtin_servers.lock().await;
    let result = match servers.as_ref() {
        Some(registry) => {
            registry
                .call_tool(server_name, tool_name, args, request_id, None)
                .await
        }
        None => {
            let request_id = value_to_json_rpc_id(
                request_id.unwrap_or_else(|| serde_json::Value::String(Uuid::new_v4().to_string())),
            );
            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32001,
                    message: "Builtin servers not initialized".to_string(),
                    data: None,
                }),
            }
        }
    };

    debug!(
        "Builtin tool call result: success={}",
        result.error.is_none()
    );

    result
}

pub async fn list_all_tools_unified(manager: &MCPServerManager) -> Result<Vec<MCPTool>> {
    let mut all_tools = Vec::new();

    // Get external server tools
    match list_all_tools(manager).await {
        Ok(external_tools) => all_tools.extend(external_tools),
        Err(e) => warn!("Failed to get external server tools: {e}"),
    }

    // Get builtin server tools
    let builtin_tools = list_builtin_tools(manager).await;
    all_tools.extend(builtin_tools);

    Ok(all_tools)
}

pub async fn call_tool_unified(
    manager: &MCPServerManager,
    server_name: &str,
    tool_name: &str,
    args: serde_json::Value,
    request_id: Option<serde_json::Value>,
) -> MCPResponse {
    // Check if it's a builtin server (starts with "builtin.")
    if server_name.starts_with("builtin.") {
        let normalized_server_name = server_name.strip_prefix("builtin.").unwrap_or(server_name);
        call_builtin_tool(manager, normalized_server_name, tool_name, args, request_id).await
    } else {
        call_tool(manager, server_name, tool_name, args, request_id).await
    }
}

pub async fn get_service_context(
    manager: &MCPServerManager,
    server_name: &str,
    options: Option<ServiceContextOptions>,
) -> Result<ServiceContext, String> {
    // Check built-in servers first
    let servers = manager.builtin_servers.lock().await;
    if let Some(registry) = servers.as_ref() {
        if let Ok(context) = registry
            .get_server_context(
                server_name,
                options.map(|o| serde_json::to_value(o).unwrap_or(Value::Null)),
            )
            .await
        {
            return Ok(context);
        }
    }

    // Fallback for external MCP servers (future implementation)
    Ok(ServiceContext {
        context_prompt: format!("# MCP Server Context\nServer ID: {server_name}\nStatus: Active"),
        structured_state: None,
    })
}

/// Get static tool definitions for ALL builtin servers without requiring runtime instantiation.
/// This provides a centralized access point for discovering all available builtin tools.
///
/// Returns a complete list of tool schemas from all 10 builtin servers:
/// - Planning (15 tools): Goal and todo management
/// - Knowledge (5 tools): Assistant-scoped knowledge base
/// - Browser (13 tools): Web browser automation
/// - Workspace (30+ tools): File operations and shell execution
/// - ContentStore (5 tools): File attachment and semantic search
/// - Assistant (4 tools): Assistant configuration management
/// - Playbook (4 tools): Playbook execution
/// - Bootstrap (2 tools): Platform and environment info
/// - UI (2 tools): User interaction prompts
/// - MCP Manager (8 tools): MCP server management
///
/// # Returns
/// A vector containing all tool schemas (88+ tools total)
pub fn get_all_static_builtin_tools() -> Vec<MCPTool> {
    let mut tools = Vec::new();

    // All servers use static tool definitions - no instantiation needed
    tools.extend(crate::mcp::builtin::planning::PlanningServer::tools_static());
    tools.extend(crate::mcp::builtin::knowledge::KnowledgeServer::tools_static());
    tools.extend(crate::mcp::builtin::browser::BrowserServer::tools_static());
    tools.extend(crate::mcp::builtin::workspace::WorkspaceServer::tools_static());
    tools.extend(crate::mcp::builtin::content_store::ContentStoreServer::tools_static());
    tools.extend(crate::mcp::builtin::assistant::AssistantServer::tools_static());
    tools.extend(crate::mcp::builtin::playbook::PlaybookServer::tools_static());

    // Stateless servers - use tools module directly
    tools.extend(crate::mcp::builtin::bootstrap::tools::all_tools());
    tools.extend(crate::mcp::builtin::ui::tools::all_tools());
    tools.extend(crate::mcp::builtin::mcp_manager::tools::all_tools());
    tools.extend(crate::mcp::builtin::session_api::tools::all_tools());
    tools.extend(crate::mcp::builtin::skills::SkillsServer::tools_static());

    tools
}

/// Get static tool definitions for a specific builtin server.
/// This provides per-server tool discovery without requiring runtime instantiation.
///
/// # Arguments
/// * `server_name` - The name of the server (e.g., "planning", "browser", "workspace")
///
/// # Returns
/// A vector of tool schemas for the specified server, or empty vector if server not found
pub fn get_static_tools_for_server(server_name: &str) -> Vec<MCPTool> {
    match server_name {
        "planning" => crate::mcp::builtin::planning::PlanningServer::tools_static(),
        "knowledge" => crate::mcp::builtin::knowledge::KnowledgeServer::tools_static(),
        "browser" => crate::mcp::builtin::browser::BrowserServer::tools_static(),
        "workspace" => crate::mcp::builtin::workspace::WorkspaceServer::tools_static(),
        "content_store" | "contentstore" => {
            crate::mcp::builtin::content_store::ContentStoreServer::tools_static()
        }
        "assistant" => crate::mcp::builtin::assistant::AssistantServer::tools_static(),
        "playbook" => crate::mcp::builtin::playbook::PlaybookServer::tools_static(),
        "bootstrap" => crate::mcp::builtin::bootstrap::tools::all_tools(),
        "ui" => crate::mcp::builtin::ui::tools::all_tools(),
        "mcp_manager" => crate::mcp::builtin::mcp_manager::tools::all_tools(),
        "session_api" => crate::mcp::builtin::session_api::tools::all_tools(),
        "skills" => crate::mcp::builtin::skills::SkillsServer::tools_static(),
        _ => Vec::new(),
    }
}
