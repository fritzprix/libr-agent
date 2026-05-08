mod lineage;
mod messages;
mod reset;
mod spawn;
mod workspace;

use crate::agent::AgentSessionManager;
use crate::session::get_session_manager;

pub use lineage::{
    lineage_store, normalize_explicit_org, remove_lineage, resolve_child_session_model_provider,
};
pub use workspace::is_restricted_system_path;

pub struct AgentService;

#[derive(Debug, Clone)]
pub struct SendSessionMessageResponse {
    pub message_id: String,
    pub status: String,
}

impl AgentService {
    /// Create a new agent session
    pub async fn create_session(
        manager: &AgentSessionManager,
        request: crate::commands::agent_commands::CreateAgentSessionRequest,
    ) -> Result<crate::repositories::SessionMetadata, String> {
        use crate::repositories::in_memory_session_repository::InMemorySessionRepository;
        use crate::repositories::SessionRepository;
        use std::sync::Arc;

        if let Some(path_str) = &request.workspace_path {
            Self::validate_and_register_workspace_override(path_str, &request.session_id).await?;
        }
        let session_repo: Arc<dyn SessionRepository> = if request.is_ephemeral {
            log::info!(
                "Creating ephemeral session (in-memory only): {}",
                request.session_id
            );
            Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>
        } else {
            log::info!(
                "Creating persistent session (DB-backed): {}",
                request.session_id
            );
            Arc::new(crate::state::get_session_repository().clone())
        };

        manager
            .create_session_with_repo(
                session_repo,
                request.session_id,
                request.name,
                request.model,
                request.provider,
                request.agent_config,
            )
            .await
    }

    /// Create a new session and IMMEDIATELY start the workflow with an initial message
    /// This is used for "Draft Mode" where the session is created only when the first message is sent.
    pub async fn create_session_with_initial_message(
        manager: &AgentSessionManager,
        request: crate::commands::agent_commands::CreateAgentSessionWithMessageRequest,
    ) -> Result<crate::commands::agent_commands::AgentResponse, String> {
        if let Some(path_str) = &request.workspace_path {
            Self::validate_and_register_workspace_override(path_str, &request.session_id).await?;
        }

        let session_repo = std::sync::Arc::new(crate::state::get_session_repository().clone());

        manager
            .create_session_with_repo(
                session_repo,
                request.session_id.clone(),
                request.name,
                request.model,
                request.provider,
                request.agent_config,
            )
            .await?;

        manager
            .start_workflow(request.session_id.clone(), request.message)
            .await
            .map(|_| crate::commands::agent_commands::AgentResponse {
                success: true,
                message: "Session created and workflow started".to_string(),
                data: None,
            })
    }

    /// Call a builtin tool directly via proxy_manager (for testing and direct execution)
    /// Returns the unwrapped MCPResult (not the full MCPResponse wrapper)
    pub async fn call_builtin_tool(
        session_id: String,
        tool_name: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::mcp::types::MCPResponseResult;
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        let response = proxy_manager
            .call_tool(&session_id, &tool_name, args)
            .await?;

        if let Some(error) = response.error {
            return Err(format!("Tool execution error: {}", error.message));
        }

        let result = response
            .result
            .ok_or_else(|| "Tool execution returned no result or error".to_string())?;

        match result {
            MCPResponseResult::ToolCall(mcp_result) => serde_json::to_value(mcp_result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e)),
            _ => Err(format!(
                "Unexpected response type for builtin tool '{}': expected ToolCall variant",
                tool_name
            )),
        }
    }

    /// Save an attachment to the session-scoped attachment store through an internal UI-only API.
    ///
    /// Routes through the session-bound `MCPServiceProxy` so that the same
    /// `AttachmentsServer` instance used by the agent is updated — keeping
    /// `recent_uploads` tracking and the BM25 search index in sync.
    pub async fn add_attachment(
        session_id: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::state::get_database_connection;
        use crate::state::get_mcp_service_proxy_manager;
        use std::sync::Arc;

        let proxy_manager = get_mcp_service_proxy_manager();

        if let Some(proxy) = proxy_manager.get_proxy(&session_id).await {
            let result = proxy
                .call_builtin_internal("attachments", "add", args)
                .await?;
            return serde_json::to_value(result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e));
        }

        log::debug!(
            "No proxy for session '{}'; falling back to direct AttachmentsServer for add_attachment",
            session_id
        );
        use crate::mcp::builtin::attachments::AttachmentsServer;

        let session_manager =
            get_session_manager().map_err(|e| format!("SessionManager not initialized: {}", e))?;
        let db = get_database_connection();
        let server = AttachmentsServer::new_with_db(
            session_id.clone(),
            Arc::new(session_manager.clone()),
            db.clone(),
        )
        .await?;
        let result = server.add_attachment_internal(args, &session_id).await?;
        serde_json::to_value(result).map_err(|e| format!("Failed to serialize MCPResult: {}", e))
    }

    /// Delete an attachment from the session-scoped attachment store through an internal UI-only API.
    ///
    /// Routes through the session-bound `MCPServiceProxy` so that the same
    /// `AttachmentsServer` instance used by the agent is updated — keeping all
    /// in-memory state consistent.
    pub async fn delete_attachment(
        session_id: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::state::get_database_connection;
        use crate::state::get_mcp_service_proxy_manager;
        use std::sync::Arc;

        let proxy_manager = get_mcp_service_proxy_manager();

        if let Some(proxy) = proxy_manager.get_proxy(&session_id).await {
            let result = proxy
                .call_builtin_internal("attachments", "delete", args)
                .await?;
            return serde_json::to_value(result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e));
        }

        log::debug!(
            "No proxy for session '{}'; falling back to direct AttachmentsServer for delete_attachment",
            session_id
        );
        use crate::mcp::builtin::attachments::AttachmentsServer;

        let session_manager =
            get_session_manager().map_err(|e| format!("SessionManager not initialized: {}", e))?;
        let db = get_database_connection();
        let server = AttachmentsServer::new_with_db(
            session_id.clone(),
            Arc::new(session_manager.clone()),
            db.clone(),
        )
        .await?;
        let result = server.delete_attachment_internal(args, &session_id).await?;
        serde_json::to_value(result).map_err(|e| format!("Failed to serialize MCPResult: {}", e))
    }

    /// Get service contexts for a session
    pub async fn get_service_contexts(
        session_id: String,
    ) -> Result<std::collections::HashMap<String, crate::mcp::types::ServiceContext>, String> {
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        proxy_manager
            .ensure_configured_proxy(&session_id, crate::state::get_app_handle().cloned())
            .await?;

        let proxy = proxy_manager
            .get_proxy(&session_id)
            .await
            .ok_or_else(|| format!("No proxy found for session: {}", session_id))?;

        Ok(proxy.get_service_contexts(None).await)
    }
}
