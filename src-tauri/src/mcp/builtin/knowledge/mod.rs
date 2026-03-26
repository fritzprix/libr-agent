use async_trait::async_trait;
use sea_orm::*;
use serde_json::Value;
use std::sync::Arc;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, MCPTool, ServiceContext};

pub mod embed;
pub mod helpers;
pub mod operations;
pub mod queries;
pub mod tools;

/// Knowledge Server v2 - Local Intelligent Memory Engine
#[derive(Debug)]
pub struct KnowledgeServer {
    #[allow(dead_code)]
    assistant_id: String,
    #[allow(dead_code)]
    db: Arc<DatabaseConnection>,
}

impl KnowledgeServer {
    /// Create a new KnowledgeServer instance for a specific assistant
    pub async fn new(assistant_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        let server = Self { assistant_id, db };
        Ok(server)
    }

    /// Get tools statically (without an instance)
    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    /// Get metadata statically (without an instance)
    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Knowledge".to_string(),
            description: "Local hybrid (Vector + FTS) knowledge base for long-term memory."
                .to_string(),
            icon: None,
        }
    }
}

pub const NAME: &str = "knowledge";

#[async_trait]
impl BuiltinMCPServer for KnowledgeServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Provides long-term memory through a local SQLite vector and graph database."
    }

    fn display_name(&self) -> String {
        "Knowledge".to_string()
    }

    fn metadata(&self) -> BuiltinServerMetadata {
        Self::metadata_static()
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
        let assistant_id = &self.assistant_id;

        match tool_name {
            "record_knowledge" => operations::record_knowledge(self, args, assistant_id).await,
            "search_knowledge" => queries::search_knowledge(self, args, assistant_id).await,
            "explore_context" => queries::explore_context(self, args, assistant_id).await,
            "prune_knowledge" => operations::prune_knowledge(self, args, assistant_id).await,
            _ => Err(format!("Tool {} not found", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let assistant_id = &self.assistant_id;

        // Lightweight summary: count entries
        // In a real implementation, we would cache this or use a lightweight count.
        // For now, let's just use a static message or a simple count if easy.

        ServiceContext {
            context_prompt: format!(
                "# Knowledge Base Context (Service: knowledge)\n\
                - **Status**: Active. Ready for Hybrid Search (FTS5 + Vector).\n\
                - **Assistant ID**: {}\n\
                Use `search_knowledge` to retrieve specific information, and `record_knowledge` to save new insights from this conversation.",
                assistant_id
            ),
            structured_state: None,
        }
    }
}
