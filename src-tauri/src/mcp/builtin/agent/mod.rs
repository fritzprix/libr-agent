use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub mod handlers;
mod formatting;
pub mod tools;

#[derive(Debug, Clone)]
struct ContextCache {
    prompt: String,
    last_update: Instant,
}

/// Agent MCP Server
#[derive(Debug)]
pub struct AgentServer {
    session_id: String,
    db: Arc<DatabaseConnection>,
    cache: Arc<RwLock<Option<ContextCache>>>,
}

impl AgentServer {
    pub async fn new(session_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        Ok(Self {
            session_id,
            db,
            cache: Arc::new(RwLock::new(None)),
        })
    }

    pub fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Agent & Session Manager".to_string(),
            description: "Manage agent configurations and orchestrate sub-agent sessions"
                .to_string(),
            icon: None,
        }
    }
}

pub const NAME: &str = "agent";

#[async_trait]
impl BuiltinMCPServer for AgentServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Manage agent configurations and orchestrate sub-agent sessions"
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let session_id = _session_id.unwrap_or_else(|| self.session_id.clone());

        let result = match tool_name {
            "create" => handlers::create_agent(self, args).await,
            "update" => handlers::update_agent(self, args, Some(session_id.clone())).await,
            "list" => handlers::list_agents_or_sessions(self, args, &session_id).await,
            "startSession" => handlers::start_session(self, args, &session_id).await,
            "messageToSession" => handlers::message_to_session(self, args, &session_id).await,
            "checkSession" => handlers::check_session(self, args, &session_id).await,
            "stopSession" => handlers::stop_session(self, args, &session_id).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        result.or_else(|e| {
            if e.contains("cancelled") || e.contains("interrupted") {
                return Err(e);
            }
            Ok(guided_error(ErrorCategory::InternalError, e, ToolGroup::Assistant).to_mcp_result())
        })
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        const CACHE_TTL: Duration = Duration::from_secs(5);

        if let Some(cache) = self.cache.read().await.as_ref() {
            if cache.last_update.elapsed() < CACHE_TTL {
                return ServiceContext {
                    context_prompt: cache.prompt.clone(),
                    structured_state: None,
                };
            }
        }

        let mut context_prompt = "# System Capability Catalog\n\n".to_string();

        use crate::mcp::builtin::service_id::BUILTIN_SERVICE_REGISTRY;
        let available_builtins: Vec<String> = BUILTIN_SERVICE_REGISTRY
            .iter()
            .filter(|e| !e.canonical.is_empty() && e.canonical != "agent" && e.canonical != "tool")
            .map(|e| e.canonical.to_string())
            .collect();

        context_prompt.push_str("### Available Builtin Capabilities\n");
        context_prompt.push_str(&format!(
            "- {}\n",
            formatting::format_capability_list(&available_builtins)
        ));
        context_prompt
            .push_str("> Grant these via `agent__update(builtinCapabilities=[...])`.\n\n");

        let mcp_repo = crate::state::get_mcp_server_repository();
        context_prompt.push_str("### Registered External MCP Servers\n");

        if let Ok(external_servers) = mcp_repo.list().await {
            context_prompt.push_str(&format!(
                "{}\n",
                formatting::format_registered_external_servers(&external_servers)
            ));
        }

        context_prompt.push_str("\nUse `agent__startSession(agentId=\"ID\", task=\"...\")` to delegate work to specialists.");

        if let Ok(mut cache) = self.cache.try_write() {
            *cache = Some(ContextCache {
                prompt: context_prompt.clone(),
                last_update: Instant::now(),
            });
        }

        ServiceContext {
            context_prompt,
            structured_state: None,
        }
    }
}
