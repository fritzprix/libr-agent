use async_trait::async_trait;
use serde_json::Value;

use super::BuiltinMCPServer;
use crate::mcp::types::MCPResult;
use crate::mcp::MCPTool;

pub mod handlers;
pub mod tools;

#[derive(Debug, Default, Clone)]
pub struct DatasetServer {}

impl DatasetServer {
    pub fn new() -> Self {
        Self {}
    }
}

pub const NAME: &str = "dataset";

#[async_trait]
impl BuiltinMCPServer for DatasetServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Dataset export and fine-tuning utilities"
    }

    fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "export_dataset" => handlers::export_dataset(args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
