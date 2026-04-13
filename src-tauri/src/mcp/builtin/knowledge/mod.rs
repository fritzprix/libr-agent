use async_trait::async_trait;
use sea_orm::*;
use serde_json::Value;
use std::sync::Arc;

use crate::entity::knowledge_chunk_v2;
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{
    BuiltinServerMetadata, ContextVolatility, MCPResult, MCPTool, ServiceContext,
};

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
        let chunk_count = knowledge_chunk_v2::Entity::find()
            .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
            .count(self.db.as_ref())
            .await
            .ok();

        ServiceContext::new(format!(
            "# Knowledge Base\n\nAssistant ID: {}\nStored Chunks: {}",
            assistant_id,
            chunk_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        ))
        .with_volatility(ContextVolatility::Medium)
    }
}
