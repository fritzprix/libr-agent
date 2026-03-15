use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

pub mod operations;
pub mod queries;
pub mod tools;

use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
struct ContextCache {
    #[allow(dead_code)]
    prompt: String,
    #[allow(dead_code)]
    last_update: Instant,
}

/// Assistant MCP Server - DEPRECATED
#[derive(Debug)]
pub struct AssistantServer {
    db: Arc<DatabaseConnection>,
    cache: Arc<RwLock<Option<ContextCache>>>,
}

impl AssistantServer {
    pub async fn new(db: Arc<DatabaseConnection>) -> Result<Self, String> {
        Ok(Self {
            db,
            cache: Arc::new(RwLock::new(None)),
        })
    }

    pub fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub(crate) async fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.try_write() {
            *cache = None;
        }
    }

    pub fn tools_static() -> Vec<MCPTool> {
        // Return empty to hide from LLM
        Vec::new()
    }

    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Assistant Manager (Legacy)".to_string(),
            description: "DEPRECATED: Use the 'agent' domain instead.".to_string(),
            icon: None,
        }
    }
}

pub const NAME: &str = "";

#[async_trait]
impl BuiltinMCPServer for AssistantServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "DEPRECATED: Use the 'agent' domain instead."
    }

    fn display_name(&self) -> String {
        "Assistant (Legacy)".to_string()
    }

    fn metadata(&self) -> BuiltinServerMetadata {
        Self::metadata_static()
    }

    fn tools(&self) -> Vec<MCPTool> {
        // Return empty to definitively hide from LLM agent
        Vec::new()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        _args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        log::warn!("Call to deprecated assistant tool: {}", tool_name);
        Err("The 'assistant' domain is deprecated. Please use the 'agent' domain instead (e.g. agent__create, agent__list).".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Return empty context to hide from system prompt
        ServiceContext {
            context_prompt: String::new(),
            structured_state: None,
        }
    }
}
