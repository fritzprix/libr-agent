// mod.rs - Module declarations and re-exports
use async_trait::async_trait;

use crate::mcp::{MCPResponse, MCPTool};

mod handlers;
mod helpers;
mod schemas;
mod server;
mod types;

// Existing modules
pub mod parsers;
pub mod search;
pub mod storage;
pub mod utils;

// Re-export public API
pub use server::ContentStoreServer;

use super::BuiltinMCPServer;
use crate::mcp::types::{ServiceContext, ServiceContextOptions};
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

    fn tools(&self) -> Vec<MCPTool> {
        self.tools()
    }

    fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        self.get_service_context(options)
    }

    async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> {
        self.switch_context(options).await
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "addContent" => self.handle_add_content(args).await,
            "listContent" => self.handle_list_content(args).await,
            "readContent" => self.handle_read_content(args).await,
            "keywordSimilaritySearch" => self.handle_keyword_search(args).await,
            _ => {
                let id = ContentStoreServer::generate_request_id();
                ContentStoreServer::error_response(
                    id,
                    -32601,
                    &format!("Unknown tool: {tool_name}"),
                )
            }
        }
    }
}
