use crate::agent::AgentSessionManager;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{BuiltinServerMetadata, ContextVolatility, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::{build_explicit_org_layer_context, SqliteSessionRepository};
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

mod formatting;
pub mod handlers;
pub mod tools;

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

    async fn legacy_assistant_server(
        &self,
    ) -> Result<crate::mcp::builtin::assistant::AssistantServer, String> {
        crate::mcp::builtin::assistant::AssistantServer::new(Arc::new(self.get_db().clone())).await
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

    async fn build_org_layer_context(&self) -> Result<Option<String>, String> {
        let repo = SqliteSessionRepository::new(self.get_db().clone());
        build_explicit_org_layer_context(&repo, &self.session_id)
            .await
            .map_err(|error| format!("Failed to load org layer context: {}", error))
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
            "createOrg" => handlers::create_org(self, args, &session_id).await,
            "getOrg" => handlers::get_org(self, args, &session_id).await,
            "startSession" => handlers::start_session(self, args, &session_id).await,
            "messageToSession" => handlers::message_to_session(self, args, &session_id).await,
            "checkSession" => handlers::check_session(self, args, &session_id).await,
            "compactSessionContext" => {
                handlers::compact_session_context(self, args, &session_id).await
            }
            "stopSession" => handlers::stop_session(self, args, &session_id).await,
            "createAssistant" => {
                let assistant_server = self.legacy_assistant_server().await?;
                crate::mcp::builtin::assistant::operations::create_assistant(
                    &assistant_server,
                    args,
                )
                .await
            }
            "updateAssistant" => {
                let assistant_server = self.legacy_assistant_server().await?;
                crate::mcp::builtin::assistant::operations::update_assistant(
                    &assistant_server,
                    args,
                    Some(session_id.clone()),
                )
                .await
            }
            "listAssistants" | "searchAssistant" | "getAssistant" => match tool_name {
                "listAssistants" => {
                    crate::mcp::builtin::assistant::queries::list_assistants(self.get_db(), args)
                        .await
                }
                "searchAssistant" => {
                    crate::mcp::builtin::assistant::queries::search_assistant(self.get_db(), args)
                        .await
                }
                "getAssistant" => {
                    crate::mcp::builtin::assistant::queries::get_assistant(self.get_db(), args)
                        .await
                }
                _ => unreachable!("legacy assistant dispatch exhaustively matched"),
            },
            "deleteAssistant" => {
                let assistant_server = self.legacy_assistant_server().await?;
                crate::mcp::builtin::assistant::operations::delete_assistant(
                    &assistant_server,
                    args,
                    Some(session_id.clone()),
                )
                .await
            }
            "healthCheck" | "spawnAgent" | "getAgentStatus" | "awaitAgent" | "getAgentLog"
            | "getChildAgents" | "messageAgent" | "terminateAgent" | "listAgentTypes"
            | "getAgentConfig" => {
                crate::mcp::builtin::session_api::handlers::handle_tool_call(
                    tool_name,
                    args,
                    Some(session_id.clone()),
                    self.get_manager(),
                )
                .await
            }
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
        let mut context_prompt = concat!(
            "# Agent Delegation\n\n",
            "- `agent__startSession` starts delegated work.\n",
            "- `agent__messageToSession` resumes or retries an existing delegated session.\n",
            "- `agent__compactSessionContext` refreshes another session's stored compact summary before more work.\n",
        )
        .to_string();

        let mut volatility = ContextVolatility::Stable;
        if let Ok(Some(org_layer_context)) = self.build_org_layer_context().await {
            context_prompt.push('\n');
            context_prompt.push_str(&org_layer_context);
            volatility = ContextVolatility::Medium;
        }

        ServiceContext::new(context_prompt).with_volatility(volatility)
    }
}
