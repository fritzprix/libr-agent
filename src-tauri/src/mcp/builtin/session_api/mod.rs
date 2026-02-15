use async_trait::async_trait;
use serde_json::{json, Value};

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext};
use crate::mcp::MCPTool;

pub mod cache;
pub mod client;
pub mod formatting;
pub mod handlers;
pub mod tools;
pub mod types;
pub mod utils;

use self::handlers::*;

#[derive(Debug, Default)]
pub struct SessionApiServer;

impl SessionApiServer {
    pub fn new() -> Self {
        Self
    }

    pub fn metadata_static() -> crate::mcp::types::BuiltinServerMetadata {
        crate::mcp::types::BuiltinServerMetadata {
            display_name: "Session API".to_string(),
            description: "Client tools for the internal Session Management HTTP API".to_string(),
            icon: None,
        }
    }

    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }
}

#[async_trait]
impl BuiltinMCPServer for SessionApiServer {
    fn name(&self) -> &str {
        "session_api"
    }

    fn description(&self) -> &str {
        "Client tools for internal HTTP Session Management API"
    }

    fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        caller_session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "healthCheck" => handle_health_check().await,
            "createSession" => handle_create_session(args, caller_session_id).await,
            "createChildSession" => handle_create_child_session(args, caller_session_id).await,
            "getSession" => handle_get_session(args).await,
            "waitForSessionIdle" => handle_wait_for_session_idle(args).await,
            "getMessages" => handle_get_messages(args, caller_session_id).await,
            "getChildSessions" => handle_get_child_sessions(args).await,
            "sendMessage" => handle_send_message(args).await,
            "terminateSession" => handle_terminate_session(args).await,
            "listAssistants" => handle_list_assistants().await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let base_url = client::base_url().await;

        ServiceContext {
            context_prompt: format!(
                "## Session API\n\nInternal API client is available at {}\nUse these tools to create/manage nested sessions.",
                base_url
            ),
            structured_state: Some(json!({
                "base_url": base_url,
                "server": "session_api"
            })),
        }
    }
}
