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
// Re-export public API
pub use server::AttachmentsServer;

use super::BuiltinMCPServer;
use crate::mcp::types::ServiceContext;
use serde_json::Value;

pub const NAME: &str = "attachments";

// BuiltinMCPServer trait implementation
#[async_trait]
impl BuiltinMCPServer for AttachmentsServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Session-scoped file attachment store (ephemeral: cleared when session ends). Use for files uploaded in the current task."
    }

    fn display_name(&self) -> String {
        "Attachments".to_string()
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
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let target_session_id = _session_id.unwrap_or_else(|| self.session_id.clone());

        match tool_name {
            // Internal UI-only writes (addAttachment, deleteAttachment — not in tools_static()).
            "addAttachment" => operations::add_content(self, args, &target_session_id).await,
            "deleteAttachment" => operations::delete_content(self, args, &target_session_id).await,
            // Agent-facing read/search tools (also in tools_static()).
            "listAttachments" => queries::list_content(self, args, &target_session_id).await,
            "readAttachment" => queries::read_content(self, args, &target_session_id).await,
            "searchAttachments" => {
                queries::keyword_similarity_search(self, args, &target_session_id).await
            }
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
