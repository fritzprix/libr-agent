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

// BuiltinMCPServer trait implementation
#[async_trait]
impl BuiltinMCPServer for ContentStoreServer {
    fn name(&self) -> &str {
        "contentstore"
    }

    fn description(&self) -> &str {
        "File attachment and semantic search system with native performance and BM25 indexing"
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
            "saveKnowledge" | "addContent" => {
                operations::save_knowledge(self, args, &target_session_id).await
            }
            "listContent" => queries::list_content(self, args, &target_session_id).await,
            "readContent" => queries::read_content(self, args, &target_session_id).await,
            "searchKnowledge" | "keywordSimilaritySearch" => {
                queries::search_knowledge(self, args, &target_session_id).await
            }
            "deleteContent" => operations::delete_content(self, args, &target_session_id).await,
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
