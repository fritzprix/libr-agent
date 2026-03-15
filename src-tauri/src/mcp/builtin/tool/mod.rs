use async_trait::async_trait;
use serde_json::{json, Value};

use super::BuiltinMCPServer;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::{MCPResult, ServiceContext};
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

fn format_capability_list(values: &[String]) -> String {
    if values.is_empty() {
        "None".to_string()
    } else {
        values.join(", ")
    }
}

fn format_registered_external_servers(
    server_models: &[crate::entity::mcp_server::Model],
) -> String {
    if server_models.is_empty() {
        "None".to_string()
    } else {
        server_models
            .iter()
            .map(|server| format!("{} (ID: {})", server.name, server.id))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

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
                return ServiceContext {
                    context_prompt: cache.prompt.clone(),
                    structured_state: Some(cache.state.clone()),
                };
            }
        }

        let mut context_prompt =
            "## Tool Management\n\nServer management tool for current session\nStatus: Ready\n\n### System Capability Catalog\n"
                .to_string();

        use crate::mcp::builtin::service_id::BUILTIN_SERVICE_REGISTRY;
        let available_builtins: Vec<String> = BUILTIN_SERVICE_REGISTRY
            .iter()
            .filter(|e| !e.canonical.is_empty() && e.canonical != "agent" && e.canonical != "tool")
            .map(|e| e.canonical.to_string())
            .collect();
        context_prompt.push_str(&format!(
            "Available Builtins: {}\n",
            format_capability_list(&available_builtins)
        ));

        let mcp_repo = crate::state::get_mcp_server_repository();
        if let Ok(external_servers) = mcp_repo.list().await {
            context_prompt.push_str(&format!(
                "Available External MCPs: {}\n",
                format_registered_external_servers(&external_servers)
            ));
        }

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

        ServiceContext {
            context_prompt,
            structured_state: Some(structured_state),
        }
    }
}
