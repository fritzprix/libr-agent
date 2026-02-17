use async_trait::async_trait;
use serde_json::{json, Value};

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext, MCPTool};

pub mod tools;
mod types;
mod client;
mod formatting;
mod cache;
mod utils;
mod handlers;

#[derive(Debug, Default)]
pub struct SessionApiServer;

impl SessionApiServer {
    const SWARM_CONTEXT_NODE_LIMIT: usize = 40;

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
        handlers::handle_tool_call(tool_name, args, caller_session_id).await
    }

    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        let base_url = client::base_url().await;

        // 1. Base prompt
        let mut context_prompt = format!(
            "## Session API\n\nInternal API client is available at {}\nUse these tools to create/manage nested sessions.",
            base_url
        );

        // 2. Fetch swarm snapshot if session_id is provided
        if let Some(opts) = options {
            if let Some(session_id) = opts.get("sessionId").and_then(|v| v.as_str()) {
                match utils::collect_descendant_snapshot(session_id, Self::SWARM_CONTEXT_NODE_LIMIT)
                    .await
                {
                    Ok((rows, truncated)) => {
                        context_prompt.push_str("\n\n### Swarm Snapshot\n");
                        context_prompt.push_str(&formatting::build_swarm_snapshot_text(
                            session_id,
                            &rows,
                            truncated,
                            Self::SWARM_CONTEXT_NODE_LIMIT,
                        ));
                        context_prompt.push_str("\n\nUse `session_api` tools to communicate with specific sub-agents or poll their messages.");
                    }
                    Err(e) => {
                        log::warn!("Failed to fetch child sessions for context: {}", e);
                    }
                }
            }
        }

        ServiceContext {
            context_prompt,
            structured_state: Some(json!({
                "base_url": base_url,
                "server": "session_api"
            })),
        }
    }
}
