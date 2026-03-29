use async_trait::async_trait;
use serde_json::Value;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, MCPTool, ServiceContext};

pub mod cache;
pub mod client;
pub mod formatting;
pub mod handlers;
pub mod tools;
pub mod types;
pub mod utils;

#[derive(Debug, Default)]
pub struct SessionApiServer;

impl SessionApiServer {
    pub fn new() -> Self {
        Self
    }

    pub fn tools_static() -> Vec<MCPTool> {
        // Return empty to hide from LLM
        Vec::new()
    }

    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Swarm Orchestrator (Legacy)".to_string(),
            description: "DEPRECATED: Use the 'agent' domain instead.".to_string(),
            icon: None,
        }
    }
}

pub const NAME: &str = "";

#[async_trait]
impl BuiltinMCPServer for SessionApiServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "DEPRECATED: Use the 'agent' domain instead."
    }

    fn display_name(&self) -> String {
        "Swarm (Legacy)".to_string()
    }

    fn metadata(&self) -> BuiltinServerMetadata {
        Self::metadata_static()
    }

    fn tools(&self) -> Vec<MCPTool> {
        // Return empty to definitively hide from LLM agent
        Vec::new()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        _args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        log::warn!("Call to deprecated swarm tool: {}", tool_name);
        Err("The 'swarm' domain is deprecated. Please use the 'agent' domain instead (e.g. agent__startSession).".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Return empty context to hide from system prompt
        ServiceContext::new(String::new())
    }
}
