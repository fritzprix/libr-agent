use async_trait::async_trait;
use sea_orm::*;
use serde_json::Value;
use std::sync::Arc;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, MCPTool, ServiceContext};

pub mod helpers;
pub mod operations;
pub mod queries;
pub mod tools;

/// Knowledge Server - DEPRECATED
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

    pub(crate) fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Get tools statically (without an instance)
    pub fn tools_static() -> Vec<MCPTool> {
        Vec::new()
    }

    /// Get metadata statically (without an instance)
    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Knowledge Server (Legacy)".to_string(),
            description: "DEPRECATED: Use an external storage MCP instead.".to_string(),
            icon: None,
        }
    }
}

pub const NAME: &str = "";

#[async_trait]
impl BuiltinMCPServer for KnowledgeServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "DEPRECATED: Use an external storage MCP instead."
    }

    fn display_name(&self) -> String {
        "Knowledge (Legacy)".to_string()
    }

    fn metadata(&self) -> BuiltinServerMetadata {
        Self::metadata_static()
    }

    fn tools(&self) -> Vec<MCPTool> {
        Vec::new()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        _args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        log::warn!("Call to deprecated knowledge tool: {}", tool_name);
        Err("The 'knowledge' domain is deprecated.".to_string())
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: String::new(),
            structured_state: None,
        }
    }
}
