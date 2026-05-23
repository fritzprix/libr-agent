use async_trait::async_trait;
use serde_json::{json, Value};

use super::BuiltinMCPServer;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::{ContextVolatility, MCPResult, ServiceContext};
use crate::mcp::MCPTool;

mod operations;
mod queries;
pub mod tools;

#[derive(Debug, Default, Clone)]
pub struct ToolServer {}

impl ToolServer {
    pub fn new() -> Self {
        Self {}
    }
}

pub const NAME: &str = "tool";

#[async_trait]
impl BuiltinMCPServer for ToolServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Manage MCP servers and connections"
    }

    fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "list" => operations::list_tools(args, session_id.as_deref()).await,
            "register" => operations::register_server(self, args).await,
            "update" => operations::update_server(self, args).await,
            "delete" => operations::delete_server(self, args).await,
            "verify" => operations::verify_server(self, args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
        .or_else(|e| {
            if e.contains("cancelled") || e.contains("interrupted") {
                return Err(e);
            }
            Ok(guided_error(ErrorCategory::InternalError, e, ToolGroup::Tool).to_mcp_result())
        })
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let context_prompt = "## Tool Management\n\nStatus: Ready";

        let structured_state = json!({ "status": "ready" });

        ServiceContext::new(context_prompt)
            .with_structured_state(structured_state)
            .with_volatility(ContextVolatility::Stable)
    }
}
