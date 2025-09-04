use crate::mcp::{MCPResponse, MCPTool};
use async_trait::async_trait;
use serde_json::Value;

pub mod workspace;
pub mod utils;

/// Trait for built-in MCP servers
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync {
    /// Server name (e.g., "builtin.filesystem")
    fn name(&self) -> &str;

    /// Server description
    #[allow(dead_code)]
    fn description(&self) -> &str;

    /// Server version
    #[allow(dead_code)]
    fn version(&self) -> &str {
        "1.0.0"
    }

    /// List available tools for this server
    fn tools(&self) -> Vec<MCPTool>;

    /// Call a tool on this server
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;
}

/// Built-in server registry
pub struct BuiltinServerRegistry {
    servers: std::collections::HashMap<String, Box<dyn BuiltinMCPServer>>,
}

impl BuiltinServerRegistry {
    pub fn new() -> Self {
        Self::with_app_handle()
    }

    pub fn with_app_handle() -> Self {
        let mut registry = Self {
            servers: std::collections::HashMap::new(),
        };

        // Create shared SecureFileManager instance
        let file_manager = std::sync::Arc::new(crate::services::SecureFileManager::new());

        // Register built-in workspace server (replaces filesystem and sandbox)
        registry.register_server(Box::new(workspace::WorkspaceServer::new(
            file_manager.clone(),
        )));

        // Browser Agent server removed to prevent duplicate tools.
        // Browser functionality now provided by frontend BrowserToolProvider.

        registry
    }

    pub fn register_server(&mut self, server: Box<dyn BuiltinMCPServer>) {
        let name = server.name().to_string();
        self.servers.insert(name, server);
    }

    pub fn get_server(&self, name: &str) -> Option<&dyn BuiltinMCPServer> {
        self.servers.get(name).map(|s| s.as_ref())
    }

    pub fn list_servers(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }

    pub fn list_all_tools(&self) -> Vec<MCPTool> {
        let mut all_tools = Vec::new();

        for server in self.servers.values() {
            let tools = server.tools();
            // Prefix tool names with server name for uniqueness
            all_tools.extend(tools);
        }

        all_tools
    }

    pub fn list_tools_for_server(&self, server_name: &str) -> Vec<MCPTool> {
        // Remove "builtin." prefix if present
        let normalized_server_name = if let Some(stripped) = server_name.strip_prefix("builtin.") {
            stripped
        } else {
            server_name
        };

        if let Some(server) = self.get_server(normalized_server_name) {
            server.tools()
        } else {
            Vec::new()
        }
    }

    pub async fn call_tool(&self, server_name: &str, tool_name: &str, args: Value) -> MCPResponse {
        if let Some(server) = self.get_server(server_name) {
            server.call_tool(tool_name, args).await
        } else {
            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(Value::String(uuid::Uuid::new_v4().to_string())),
                result: None,
                error: Some(crate::mcp::MCPError {
                    code: -32601,
                    message: format!("Built-in server '{server_name}' not found"),
                    data: None,
                }),
            }
        }
    }
}

impl Default for BuiltinServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
