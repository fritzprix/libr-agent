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

use client::SessionApiClient;

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
        let client = SessionApiClient::new();

        match tool_name {
            "healthCheck" => handlers::handle_health_check(&client).await,
            "createSession" => {
                handlers::handle_create_session(&client, args, caller_session_id).await
            }
            "createChildSession" => {
                handlers::handle_create_child_session(&client, args, caller_session_id).await
            }
            "getSession" => handlers::handle_get_session(&client, args).await,
            "waitForSessionIdle" => handlers::handle_wait_for_session_idle(&client, args).await,
            "getMessages" => {
                handlers::handle_get_messages(&client, args, caller_session_id).await
            }
            "getChildSessions" => handlers::handle_get_child_sessions(&client, args).await,
            "sendMessage" => handlers::handle_send_message(&client, args).await,
            "terminateSession" => handlers::handle_terminate_session(&client, args).await,
            "listAssistants" => handlers::handle_list_assistants(&client).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let client = SessionApiClient::new();
        let base_url = client.base_url().await;

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
