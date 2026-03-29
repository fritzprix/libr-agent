use async_trait::async_trait;
use serde_json::{json, Value};

use super::BuiltinMCPServer;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::{ContextVolatility, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::mcp_server_repository::MCPServerRepository;

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
pub struct ToolServer {
    cache: Arc<RwLock<Option<ContextCache>>>,
}

impl ToolServer {
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

pub const NAME: &str = "tool";

#[async_trait]
impl BuiltinMCPServer for ToolServer {
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
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "list" | "listTools" => operations::list_tools(args).await,
            "register" | "registerServer" => operations::register_server(self, args).await,
            "update" | "updateServer" => operations::update_server(self, args).await,
            "delete" | "deleteServer" => operations::delete_server(self, args).await,
            "verify" | "verifyServer" => operations::verify_server(self, args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
        .or_else(|e| {
            if e.contains("cancelled") || e.contains("interrupted") {
                return Err(e);
            }
            Ok(guided_error(ErrorCategory::InternalError, e, ToolGroup::Tool).to_mcp_result())
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

        let mut context_prompt =
            "## Tool Management\n\nServer management tool for current session\nStatus: Ready\n\n### System Capability Reference\n"
                .to_string();

        use crate::mcp::builtin::service_id::BUILTIN_SERVICE_REGISTRY;
        let available_builtins_count = BUILTIN_SERVICE_REGISTRY
            .iter()
            .filter(|e| !e.canonical.is_empty() && e.canonical != "agent" && e.canonical != "tool")
            .count();
        context_prompt.push_str(
            "Reference only. The items below describe platform-level inventory and may not be enabled in this session.\n",
        );
        context_prompt.push_str(&format!(
            "- Builtin capability families installed: {}\n",
            available_builtins_count
        ));

        let mcp_repo = crate::state::get_mcp_server_repository();
        if let Ok(external_servers) = mcp_repo.list().await {
            context_prompt.push_str(&format!(
                "- External MCP server registrations: {}\n",
                external_servers.len()
            ));
        }
        context_prompt.push_str(
            "- Use `tool__list` to inspect builtin tools and saved external server inventories.\n\
             - Use `tool__verify` if you need to confirm a registered external server is healthy.\n",
        );

        let structured_state = json!({
            "mode": "session-isolated",
            "note": "External servers are managed per-session through MCPServiceProxy"
        });

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
