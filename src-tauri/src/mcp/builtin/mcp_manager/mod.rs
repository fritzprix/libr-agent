use async_trait::async_trait;
use serde_json::{json, Value};

use super::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext};

use crate::mcp::MCPTool;
use crate::state::get_mcp_manager;

mod operations;
mod queries;

#[derive(Debug, Default, Clone)]
pub struct MCPManagerServer;

impl MCPManagerServer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl BuiltinMCPServer for MCPManagerServer {
    fn name(&self) -> &str {
        "mcp_manager"
    }

    fn description(&self) -> &str {
        "Manage MCP servers and connections"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "listServers".to_string(),
                title: Some("List Servers".to_string()),
                description: "List all registered MCP servers
                
⚠️ CRITICAL WORKFLOW:
1. Call this tool to see available servers and their status
2. Use the 'name' from the list for other operations
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "page": { "type": "integer", "minimum": 1, "description": "Page number for pagination" },
                        "pageSize": { "type": "integer", "minimum": -1, "description": "Number of items per page" },
                        "filterByAssistant": { "type": "boolean", "description": "Filter servers by assistant capability" },
                        "includeInactive": { "type": "boolean", "description": "Include inactive/disconnected servers" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "searchServer".to_string(),
                title: Some("Search Server".to_string()),
                description: "Search for MCP servers by name".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "searchMode": { "type": "string", "enum": ["simple", "bm25"], "description": "Search mode (simple or bm25)" },
                        "weights": { 
                            "type": "object", 
                            "properties": {
                                "name": { "type": "number" },
                                "description": { "type": "number" }
                            },
                            "description": "Search weights for fields"
                        }
                    },
                    "required": ["query"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "createServer".to_string(),
                title: Some("Create Server".to_string()),
                description: "Register and start a new MCP server.

⚠️ CRITICAL WORKFLOW:
1. Verify the 'name' is unique using searchServer or listServers
2. For stdio servers, verify the command exists using 'workspace.executeCommand' if possible
3. Provide the full 'transport' configuration
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1, "description": "Unique name for the server" },
                        "transport": {
                            "type": "object",
                            "description": "Transport configuration",
                            "properties": {
                                "type": { "type": "string", "enum": ["stdio", "http"], "description": "Transport type" },
                                "command": { "type": "string", "description": "Command to execute (stdio only)" },
                                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments (stdio only)" },
                                "env": { "type": "object", "description": "Environment variables" },
                                "url": { "type": "string", "description": "Server URL (http only)" },
                                "headers": { "type": "object", "description": "HTTP headers" }
                            },
                            "required": ["type"]
                        }
                    },
                    "required": ["name", "transport"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "updateServer".to_string(),
                title: Some("Update Server".to_string()),
                description: "Update configuration for an existing MCP server.
                
⚠️ CRITICAL WORKFLOW:
1. Call get_server_config logic (via listServers) to see current config isn't directly exposed, but use listServers to verify existence.
2. Provide the 'name' exactly as listed.
3. This operation will restart the server if it is currently running.
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Name of the server to update" },
                        "transport": {
                            "type": "object",
                            "description": "New transport configuration",
                            "properties": {
                                "type": { "type": "string", "enum": ["stdio", "http"], "description": "Transport type" },
                                "command": { "type": "string", "description": "Command to execute (stdio only)" },
                                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments (stdio only)" },
                                "env": { "type": "object", "description": "Environment variables" },
                                "url": { "type": "string", "description": "Server URL (http only)" },
                                "headers": { "type": "object", "description": "HTTP headers" }
                            },
                            "required": ["type"]
                        }
                    },
                    "required": ["name", "transport"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "deleteServer".to_string(),
                title: Some("Delete Server".to_string()),
                description: "Delete an MCP server configuration.
                
⚠️ WARNING: This action is permanent.
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Name of the server to delete" }
                    },
                    "required": ["name"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "connectServer".to_string(),
                title: Some("Connect Server".to_string()),
                description: "Connect to an existing MCP server.
                
⚠️ CRITICAL WORKFLOW:
1. Use listServers to check status first.
2. Use this if status is 'disconnected'.
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "serverName": { "type": "string", "description": "Name of the server to connect" },
                        "serverId": { "type": "string", "description": "ID of the server (optional)" },
                        "scope": { "type": "string", "description": "Connection scope (optional)" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "disconnectServer".to_string(),
                title: Some("Disconnect Server".to_string()),
                description: "Disconnect an MCP server.
                
⚠️ CRITICAL WORKFLOW:
1. Use listServers to check status first.
2. Use this if status is 'connected'.
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "serverName": { "type": "string", "description": "Name of the server to disconnect" },
                        "serverId": { "type": "string", "description": "ID of the server (optional)" },
                        "scope": { "type": "string", "description": "Connection scope (optional)" }
                    }
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
        ]
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "listServers" => queries::list_servers(args).await,
            "searchServer" => queries::search_server(args).await,
            "createServer" => operations::create_server(args).await,
            "updateServer" => operations::update_server(args).await,
            "deleteServer" => operations::delete_server(args).await,
            "connectServer" => operations::connect_server(args).await,
            "disconnectServer" => operations::disconnect_server(args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let manager = get_mcp_manager();
        let count = manager.connections.lock().await.len();

        ServiceContext {
            context_prompt: format!("MCP Manager: {} active servers", count),
            structured_state: Some(json!({
                "active_servers_count": count
            })),
        }
    }
}
