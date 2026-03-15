use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::AssistantRepository;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub mod handlers;
pub mod tools;

#[derive(Debug, Clone)]
struct ContextCache {
    prompt: String,
    last_update: Instant,
}

/// Agent MCP Server
///
/// Unified domain for managing agent configurations (Assistants) and
/// orchestrating agent sessions (Swarm).
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

    pub(crate) async fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.try_write() {
            *cache = None;
        }
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

        let repo = crate::repositories::SqliteAssistantRepository::new(self.get_db().clone());
        let total_agents = repo.count_assistants().await.unwrap_or(0);
        let agents = repo.list_assistants().await.unwrap_or_default();

        let mut context_prompt = format!(
            "# Agent System Status\n\
            **Status**: Active\n\
            **Total Agent Configs**: {}\n\n\
            ### Available Specialized Agents\n",
            total_agents
        );

        if agents.is_empty() {
            context_prompt
                .push_str("*No agents configured yet. Use `agent__create` to add one.*\n");
        } else {
            for agent in agents.iter().take(5) {
                let config: Value = serde_json::from_str(&agent.config).unwrap_or_default();
                let desc = config
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No description");
                let builtins = config
                    .get("allowedBuiltInServiceAliases")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let externals = config
                    .get("mcpServerIds")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);

                context_prompt.push_str(&format!(
                    "- **{}** (ID: `{}`): {} [{} builtins, {} external MCPs]\n",
                    agent.name, agent.id, desc, builtins, externals
                ));
            }
            if total_agents > 5 {
                context_prompt.push_str(&format!(
                    "*...and {} more. Use `agent__list(type='configs')` for full list.*\n",
                    total_agents - 5
                ));
            }
        }

        context_prompt.push_str("\nUse `agent__startSession(agentId=\"ID\", task=\"...\")` to delegate work to these specialists.");

        // Try to add sub-session info
        use crate::mcp::builtin::session_api::formatting::build_swarm_snapshot_text;
        use crate::mcp::builtin::session_api::utils::collect_descendant_snapshot;

        if let Ok((rows, truncated)) = collect_descendant_snapshot(&self.session_id, 20).await {
            if !rows.is_empty() {
                context_prompt.push_str("\n\n### Active Sub-Agent Sessions\n");
                let snapshot_text =
                    build_swarm_snapshot_text(&self.session_id, &rows, truncated, 20);
                // Neutralize text
                context_prompt
                    .push_str(&snapshot_text.replace("Swarm Board", "Agent Session Roster"));
            }
        }

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
