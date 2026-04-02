use async_trait::async_trait;
use serde_json::{json, Value};

use super::BuiltinMCPServer;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::{ContextVolatility, MCPResult, ServiceContext};

use crate::mcp::MCPTool;

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

mod operations;
mod queries;
pub mod tools;

#[derive(Debug, Clone)]
struct ContextCache {
    prompt: String,
    state: Value,
    last_update: Instant,
}

#[derive(Debug, Default, Clone)]
pub struct MCPManagerServer {
    cache: Arc<RwLock<Option<ContextCache>>>,
}

impl MCPManagerServer {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
        }
    }

    pub(crate) async fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.try_write() {
            *cache = None;
        }
    }
}

pub const NAME: &str = "mcp_manager";

#[async_trait]
impl BuiltinMCPServer for MCPManagerServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Manage MCP servers and connections"
    }

    fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "listTools" => operations::list_tools(args, session_id.as_deref()).await,
            "registerServer" => operations::register_server(self, args).await,
            "updateServer" => operations::update_server(self, args).await,
            "deleteServer" => operations::delete_server(self, args).await,
            "verifyServer" => operations::verify_server(self, args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
        .or_else(|e| {
            if e.contains("cancelled") || e.contains("interrupted") {
                return Err(e);
            }
            Ok(
                guided_error(ErrorCategory::InternalError, e, ToolGroup::McpManager)
                    .to_mcp_result(),
            )
        })
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        const CACHE_TTL: Duration = Duration::from_secs(5);

        if let Some(cache) = self.cache.read().await.as_ref() {
            if cache.last_update.elapsed() < CACHE_TTL {
                return ServiceContext::new(cache.prompt.clone())
                    .with_structured_state(cache.state.clone())
                    .with_volatility(ContextVolatility::Stable);
            }
        }

        // Note: Service Isolation prevents access to global external server state
        // The mcp_manager tool now operates per-session through MCPServiceProxy
        let context_prompt =
            "## MCP Manager\n\nServer management tool for current session\nStatus: Ready"
                .to_string();
        let structured_state = json!({
            "mode": "session-isolated",
            "note": "External servers are managed per-session through MCPServiceProxy"
        });

        // Update cache
        if let Ok(mut cache) = self.cache.try_write() {
            *cache = Some(ContextCache {
                prompt: context_prompt.clone(),
                state: structured_state.clone(),
                last_update: Instant::now(),
            });
        }

        ServiceContext::new(context_prompt)
            .with_structured_state(structured_state)
            .with_volatility(ContextVolatility::Stable)
    }
}
