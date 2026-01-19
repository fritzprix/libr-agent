use async_trait::async_trait;
use serde_json::{json, Value};

use super::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext};

use crate::mcp::MCPTool;

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

mod operations;
mod queries;

#[derive(Debug, Clone)]
struct ContextCache {
    prompt: String,
    state: Value,
    last_update: Instant,
}

#[derive(Debug, Default, Clone)]
pub struct MCPManagerServer {
    cache: Arc<RwLock<Option<ContextCache>>>,
}

impl MCPManagerServer {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
        }
    }

    pub(crate) async fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.try_write() {
            *cache = None;
        }
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
                
⚠️ MANDATORY:
1. Extract the 'name' from the list for subsequent target operations.
2. Use this tool if server status is unknown.
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "page": { "type": "integer", "minimum": 1, "description": "Page number for pagination" },
                        "pageSize": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Items per page (max 50)" },
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
                        "query": { "type": "string", "description": "Search target query" },
                        "page": { "type": "integer", "minimum": 1, "description": "Page number for pagination" },
                        "pageSize": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Items per page (max 50)" },
                        "searchMode": { "type": "string", "enum": ["simple", "bm25"], "description": "Search mode (simple or bm25)" },
                        "weights": { 
                            "type": "object", 
                            "properties": {
                                "name": { "type": "number" },
                                "description": { "type": "number" }
                            },
                            "description": "Target search weights for fields"
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

⚠️ PREREQUISITES:
1. Verify target command exists before registration (stdio servers)
2. For NPM packages: Use 'npx -y <package>' (auto-installs on-demand)
3. For Python: Use 'uvx' or direct 'python -m' if installed
4. For Docker: Use 'docker run' with appropriate image

NAMING (REQUIRED):
• Provide human-readable 'name' (e.g., 'filesystem-workspace', 'github-api')
• Must be unique across all servers
• Use descriptive names for easy identification

RETURNS:
• Server name for subsequent management operations
• Connection status

EXAMPLE:
  name: 'filesystem-workspace'
  transport:
    type: 'stdio'
    command: 'npx'
    args: ['-y', '@modelcontextprotocol/server-filesystem', '/workspace']
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Human-readable server name (e.g., 'filesystem-workspace', 'github-api'). Must be unique.",
                            "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_-]{0,62}$",
                            "minLength": 1,
                            "maxLength": 63
                        },
                        "transport": {
                            "type": "object",
                            "description": "Transport configuration",
                            "properties": {
                                "type": { "type": "string", "enum": ["stdio", "http"], "description": "Transport type" },
                                "command": { "type": "string", "description": "Command to execute (stdio only). Use 'npx' for NPM packages, 'uvx' for Python, 'docker' for containers. NEVER 'npm' or 'pip' install commands." },
                                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments (stdio only). For npx, start with '-y' flag: ['-y', '@modelcontextprotocol/server-*', ...args]" },
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

⚠️ PREREQUISITES:
1. Use listServers or searchServer to extract the target server 'name' (ID)
2. Server will restart automatically if currently running
3. For NPM packages: Use 'npx -y <package>' pattern

Returns:
• Update status
• Restart result if applicable
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Target name of the server to update" },
                        "transport": {
                            "type": "object",
                            "description": "New transport configuration",
                            "properties": {
                                "type": { "type": "string", "enum": ["stdio", "http"], "description": "Transport type" },
                                "command": { "type": "string", "description": "Command to execute (stdio only). Use 'npx' for NPM packages, 'uvx' for Python, 'docker' for containers. NEVER 'npm' or 'pip' install commands." },
                                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments (stdio only). For npx, start with '-y' flag: ['-y', '@modelcontextprotocol/server-*', ...args]" },
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
                        "name": { "type": "string", "description": "Target name of the server to exclude from configuration" }
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
                
⚠️ MANDATORY:
1. Extract the 'name' from 'listServers' FIRST.
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Target server name" }
                    },
                    "required": ["name"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },
            MCPTool {
                name: "disconnectServer".to_string(),
                title: Some("Disconnect Server".to_string()),
                description: "Disconnect an MCP server.
                
⚠️ MANDATORY:
1. Extract the 'name' from 'listServers' FIRST.
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Target server name" }
                    },
                    "required": ["name"]
                }))
                .unwrap(),
                output_schema: None,
                annotations: None,
            },

            MCPTool {
                name: "listBuiltinTools".to_string(),
                title: Some("List Builtin Tools".to_string()),
                description: "List all available built-in MCP tools across all servers.
                
⚠️ USEFUL FOR DISCOVERY:
1. Use this to find available capabilities (file ops, browser, etc.).
".to_string(),
                input_schema: serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "serverName": { "type": "string", "description": "Optional: Filter by server name (e.g. 'workspace')"}
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
            "createServer" => operations::create_server(self, args).await,
            "updateServer" => operations::update_server(self, args).await,
            "deleteServer" => operations::delete_server(self, args).await,
            "connectServer" => operations::connect_server(self, args).await,
            "disconnectServer" => operations::disconnect_server(self, args).await,
            "listBuiltinTools" => queries::list_builtin_tools(args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        const CACHE_TTL: Duration = Duration::from_secs(5);

        if let Some(cache) = self.cache.read().await.as_ref() {
            if cache.last_update.elapsed() < CACHE_TTL {
                return ServiceContext {
                    context_prompt: cache.prompt.clone(),
                    structured_state: Some(cache.state.clone()),
                };
            }
        }

        // Note: Service Isolation prevents access to global external server state
        // The mcp_manager tool now operates per-session through MCPServiceProxy
        let context_prompt =
            "## MCP Manager\n\nServer management tool for current session\nStatus: Ready"
                .to_string();
        let structured_state = json!({
            "mode": "session-isolated",
            "note": "External servers are managed per-session through MCPServiceProxy"
        });

        // Update cache
        if let Ok(mut cache) = self.cache.try_write() {
            *cache = Some(ContextCache {
                prompt: context_prompt.clone(),
                state: structured_state.clone(),
                last_update: Instant::now(),
            });
        }

        ServiceContext {
            context_prompt,
            structured_state: Some(structured_state),
        }
    }
}
