use crate::agent::AgentSessionManager;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, ContextVolatility, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::{
    build_explicit_org_layer_context, format_active_sessions_notice, SessionRepository,
    SqliteSessionRepository,
};
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

mod formatting;
pub mod handlers;
pub mod tools;
pub mod utils;

/// Agent MCP Server
#[derive(Debug)]
pub struct AgentServer {
    session_id: String,
    db: Arc<DatabaseConnection>,
    manager: Option<AgentSessionManager>,
}

impl AgentServer {
    pub async fn new(
        session_id: String,
        db: Arc<DatabaseConnection>,
        manager: Option<AgentSessionManager>,
    ) -> Result<Self, String> {
        Ok(Self {
            session_id,
            db,
            manager,
        })
    }

    pub fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn get_manager(&self) -> Option<&AgentSessionManager> {
        self.manager.as_ref()
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

pub const AGENT_DELEGATION_HEADER: &str = concat!(
    "## Agent Delegation\n\n",
    "- `agent__prepareTeamworkWorkspace` returns an app-local teamwork artifact directory for orchestration files without changing the current session workspace.\n",
    "- `agent__startSession` starts delegated work.\n",
    "- `agent__messageToSession` resumes or retries an existing delegated session.\n",
    "- `agent__compactSessionContext` refreshes another session's stored compact summary before more work.\n",
);

/// Compact header used with live child/org inventory (Volatile path).
/// Tool names live in the MCP tool list — do not restate them here.
const AGENT_DELEGATION_LIVE_PREFIX: &str = "## Agent Delegation\n";

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
            "createAgent" => handlers::create_agent(self, args).await,
            "updateAgent" => handlers::update_agent(self, args, Some(session_id.clone())).await,
            "listAgents" => handlers::list_agents_or_sessions(self, args, &session_id).await,
            "prepareTeamworkWorkspace" => {
                handlers::prepare_teamwork_workspace(self, args, &session_id).await
            }
            "createOrg" => handlers::create_org(self, args, &session_id).await,
            "getOrg" => handlers::get_org(self, args, &session_id).await,
            "startSession" => handlers::start_session(self, args, &session_id).await,
            "messageToSession" => handlers::message_to_session(self, args, &session_id).await,
            "checkSession" => handlers::check_session(self, args, &session_id).await,
            "compactSessionContext" => {
                handlers::compact_session_context(self, args, &session_id).await
            }
            "stopSession" => handlers::stop_session(self, args, &session_id).await,
            "deleteSession" => handlers::delete_session(self, args, &session_id).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        result.or_else(|e| {
            if e.contains("cancelled") || e.contains("interrupted") {
                return Err(e);
            }
            Ok(guided_error(ErrorCategory::InternalError, e, ToolGroup::Agent).to_mcp_result())
        })
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Static tool catalogue → Stable (cacheable prefix). Live child/org inventory →
        // Volatile so it only appears in the synthetic session-context user message.
        let mut live_parts: Vec<String> = Vec::new();

        let repo = SqliteSessionRepository::new(self.get_db().clone());
        if let Ok(Some(session)) = repo.get_session(&self.session_id).await {
            if let Ok(children) = repo.get_child_sessions(&self.session_id).await {
                if let Some(active_notice) = format_active_sessions_notice(&children) {
                    live_parts.push(active_notice);
                }
            }

            if session.org_id.is_some() {
                if let Ok(Some(org_layer_context)) =
                    build_explicit_org_layer_context(&repo, &session).await
                {
                    live_parts.push(org_layer_context);
                }
            }
        }

        if live_parts.is_empty() {
            return ServiceContext::new(AGENT_DELEGATION_HEADER.to_string())
                .with_volatility(ContextVolatility::Stable);
        }

        let mut context_prompt = AGENT_DELEGATION_LIVE_PREFIX.to_string();
        context_prompt.push('\n');
        context_prompt.push_str(&live_parts.join("\n\n"));

        ServiceContext::new(context_prompt).with_volatility(ContextVolatility::Volatile)
    }
}
