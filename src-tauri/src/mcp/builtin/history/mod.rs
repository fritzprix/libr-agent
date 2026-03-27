use async_trait::async_trait;
use serde_json::Value;

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, MCPTool, ServiceContext};

mod handlers;
mod tools;
mod types;

pub const NAME: &str = "history";

#[derive(Debug)]
pub struct HistoryServer {
    session_id: String,
}

impl HistoryServer {
    pub async fn new(
        session_id: String,
        _db: std::sync::Arc<sea_orm::DatabaseConnection>,
    ) -> Result<Self, String> {
        Ok(Self { session_id })
    }

    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "History".to_string(),
            description: "Read session history with paginated session, message, and search access"
                .to_string(),
            icon: None,
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for HistoryServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Read session history with paginated access to sessions, messages, and search results"
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let caller_session_id = session_id.unwrap_or_else(|| self.session_id.clone());

        let result = match tool_name {
            "list" => handlers::list_sessions(self, args).await,
            "readSession" => handlers::read_session(self, args).await,
            "readMessage" => handlers::read_message(self, args).await,
            "search" => handlers::search_history(self, args, &caller_session_id).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        result.or_else(|e| {
            Ok(guided_error(ErrorCategory::InternalError, e, ToolGroup::Agent).to_mcp_result())
        })
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: String::new(),
            structured_state: None,
        }
    }

    async fn has_active_state(&self) -> bool {
        false
    }
}
