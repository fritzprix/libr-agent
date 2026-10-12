use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{
    BuiltinServerMetadata, ContextVolatility, MCPResult, MCPTool, ServiceContext,
};
use crate::repositories::SqliteKnowledgeV2Repository;

pub mod embed;
pub mod extraction;
pub mod helpers;
pub mod operations;
pub mod queries;
pub mod tools;

/// Knowledge Server v2 - Local Intelligent Memory Engine
#[derive(Debug)]
pub struct KnowledgeServer {
    assistant_id: String,
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

    pub(crate) fn repository(&self) -> SqliteKnowledgeV2Repository {
        SqliteKnowledgeV2Repository::new(self.db.as_ref().clone())
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
            "recordKnowledge" => operations::record_knowledge(self, args, assistant_id).await,
            "searchKnowledge" => queries::search_knowledge(self, args, assistant_id).await,
            "exploreContext" => queries::explore_context(self, args, assistant_id).await,
            "pruneKnowledge" => operations::prune_knowledge(self, args, assistant_id).await,
            _ => Err(format!("Tool {} not found", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Keep this prompt static (no live counts / assistant IDs) so LLM prefix
        // caching stays warm across record/prune churn (Tool Design Manifesto Rule 6).
        ServiceContext::new(
            "# Knowledge Base\n\n\
             Persistent assistant-scoped memory. Use knowledge__searchKnowledge to retrieve \
             entries, knowledge__recordKnowledge to store durable facts, and \
             knowledge__exploreContext for graph neighborhood around a known entity."
                .to_string(),
        )
        .with_volatility(ContextVolatility::Stable)
    }
}
