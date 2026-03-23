// mod.rs - Module declarations and re-exports
use async_trait::async_trait;

use crate::mcp::types::MCPResult;
use crate::mcp::MCPTool;

mod operations;
mod queries;

// Existing modules

mod helpers;
pub mod parsers;
mod schemas;
pub mod search;
mod server;
pub mod storage;
mod types;
pub mod utils;

// Re-export public API
pub use server::ContentStoreServer;

use super::BuiltinMCPServer;
use crate::mcp::types::ServiceContext;
use serde_json::Value;

pub const NAME: &str = "attachments";

// BuiltinMCPServer trait implementation
#[async_trait]
impl BuiltinMCPServer for ContentStoreServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Session-scoped file attachment store (ephemeral: cleared when session ends). Use for files uploaded in the current task. For persistent knowledge across sessions, use the knowledge server."
    }

    fn display_name(&self) -> String {
        "Content Store".to_string()
    }

    fn tools(&self) -> Vec<MCPTool> {
        self.tools()
    }

    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        self.get_service_context(options).await
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let target_session_id = _session_id.unwrap_or_else(|| self.session_id.clone());

        match tool_name {
            // Internal UI-only write paths.
            // These are intentionally NOT included in `tools_static()`, so agents cannot
            // discover or call them, but session-bound internal callers (via proxy) can
            // still reuse the same server instance and keep in-memory state synchronized.
            "add" => operations::add_content(self, args, &target_session_id).await,
            "delete" => operations::delete_content(self, args, &target_session_id).await,
            "list" => queries::list_content(self, args, &target_session_id).await,
            "read" => queries::read_content(self, args, &target_session_id).await,
            "search" => queries::keyword_similarity_search(self, args, &target_session_id).await,
            _ => Err(format!("Unknown tool: {tool_name}")),
        }
    }
}

#[cfg(test)]
mod test_functional;
#[cfg(test)]
mod test_migration;
#[cfg(test)]
mod test_recent_uploads;
// V1 switch_context test - obsolete in V2 session-per-proxy architecture
// #[cfg(test)]
// mod test_session_isolation;
