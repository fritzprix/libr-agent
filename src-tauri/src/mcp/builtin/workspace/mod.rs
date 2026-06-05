use async_trait::async_trait;
use serde_json::Value;

use super::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext};
use crate::mcp::MCPTool;

// Module declarations
pub mod code_execution;
pub mod context;
pub mod dispatch;
pub mod export_operations;
pub mod file_operations;
pub mod handlers;
pub mod persistent_shell;
pub mod terminal_manager;
pub mod tools;
pub mod types;
pub mod ui_resources;
pub mod utils;
pub mod workspace_server;

// Re-exports for public API stability
pub use types::{
    InteractiveShellInputType, PendingExecutionLookupError, PendingExecutions,
    PendingShellExecution, PendingShellInputResolution, PERSISTENT_SHELL_TOOL, RUN_SHELL_TOOL,
};
pub(crate) use types::{
    CANCEL_INTERACTIVE_SHELL_INPUT_INTERNAL, INTERACTIVE_SHELL_INPUT_MAX_BYTES,
    INTERACTIVE_SHELL_INPUT_TIMEOUT_SECS, SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL,
};
pub use workspace_server::WorkspaceServer;

#[cfg(test)]
mod test_output_visibility;

pub const NAME: &str = "workspace";

#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Integrated workspace for file operations and code execution\n\nInternal paths: .libragent/tmp/ (process outputs), .libragent/exports/ (exported files). These are hidden from listDir/search/export operations to keep user workspace clean. Do not reference them as inputs."
    }

    fn display_name(&self) -> String {
        "Workspace".to_string()
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        self.get_service_context_internal(options).await
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        self.call_tool(tool_name, args, session_id).await
    }
}
