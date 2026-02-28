use async_trait::async_trait;
use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::{ErrorCategory, ToolGroup, guided_error};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::error_normalization::{ExternalMcpErrorCategory, categorize_session_api_error};
use crate::mcp::types::{MCPResult, MCPTool, ServiceContext};

mod cache;
mod client;
mod formatting;
mod handlers;
pub mod tools;
mod types;
mod utils;

#[derive(Debug, Default)]
pub struct SessionApiServer;

impl SessionApiServer {
    const SWARM_CONTEXT_NODE_LIMIT: usize = 40;

    pub fn new() -> Self {
        Self
    }

    pub fn metadata_static() -> crate::mcp::types::BuiltinServerMetadata {
        crate::mcp::types::BuiltinServerMetadata {
            display_name: "Swarm".to_string(),
            description: "Spawn and orchestrate child agents to delegate tasks in parallel"
                .to_string(),
            icon: None,
        }
    }

    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }
}

pub const NAME: &str = "swarm";

#[async_trait]
impl BuiltinMCPServer for SessionApiServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Spawn and orchestrate child agents to delegate tasks in parallel"
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
        handlers::handle_tool_call(tool_name, args, caller_session_id)
            .await
            .or_else(|e| {
                // Cancellation errors must propagate as Err so the workflow loop
                // can handle them correctly (abort, surface to user, etc.).
                if e.contains("cancelled") || e.contains("interrupted") {
                    return Err(e);
                }

                // Map the raw error to an error_guidance ErrorCategory so the
                // format matches planning/knowledge/browser builtins.
                let (norm_category, _) = categorize_session_api_error(&e);
                let category = match norm_category {
                    ExternalMcpErrorCategory::NotFound
                    | ExternalMcpErrorCategory::SessionExpired => ErrorCategory::ResourceNotFound,
                    ExternalMcpErrorCategory::InvalidInput => ErrorCategory::InvalidInput,
                    ExternalMcpErrorCategory::PermissionDenied => ErrorCategory::PermissionDenied,
                    ExternalMcpErrorCategory::Timeout => ErrorCategory::Timeout,
                    ExternalMcpErrorCategory::Transport => ErrorCategory::NetworkError,
                    ExternalMcpErrorCategory::Protocol
                    | ExternalMcpErrorCategory::RemoteToolError
                    | ExternalMcpErrorCategory::Internal => ErrorCategory::InternalError,
                };

                Ok(guided_error(category, e, ToolGroup::Swarm).to_mcp_result())
            })
    }

    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        let base_url = client::base_url().await;

        // 1. Base prompt
        let mut context_prompt = format!(
            "## Swarm Capability\n\nYou can delegate tasks to specialist agents and collect their results. Use this when the task benefits from parallelism or specialist knowledge. Internal API at {}",
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
                        context_prompt.push_str("\n\nUse swarm tools to communicate with specific sub-agents or poll their messages.");
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
                "server": "swarm"
            })),
        }
    }
}
