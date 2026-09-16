mod handlers;
pub mod tools;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use async_trait::async_trait;
use serde_json::Value;

pub const NAME: &str = "desktop";

/// Desktop MCP Server
///
/// Provides OS-level computer use capabilities (mouse clicks, movement, drag,
/// keyboard typing, key shortcuts, scrolling, and cursor queries) to allow agents
/// to interact directly with GUI desktop applications.
#[derive(Debug, Default, Clone)]
pub struct DesktopServer;

impl DesktopServer {
    /// Create a new `DesktopServer` instance.
    pub fn new() -> Self {
        Self
    }

    /// Get tools statically (without an instance).
    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    /// Get metadata statically.
    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Desktop".to_string(),
            description: "Control desktop mouse and keyboard to interact with GUI applications"
                .to_string(),
            icon: None,
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for DesktopServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Control desktop mouse and keyboard to interact with GUI applications"
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        log::debug!("Desktop server tool called: {}", tool_name);

        match tool_name {
            "computerControl" => handlers::handle_computer_control(args).await,
            _ => Err(format!("Unknown tool: {tool_name}")),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Desktop server has no persistent state to inject in system prompt.
        ServiceContext::new(String::new())
    }
}
