use super::contracts::{
    AgentOpenSessionResponse, AgentResponse, AgentSessionListResponse, CreateAgentSessionRequest,
    CreateAgentSessionWithMessageRequest, ListAgentSessionsRequest, PendingApprovalSnapshot,
    UpdateAgentConfigRequest,
};
use crate::agent::{AgentSessionManager, ExecutionMode};
use crate::mcp::types::{MCPTool, ServiceContext};
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionMetadata;
use crate::services::agent_service::remove_lineage;
use crate::services::AgentService;
use crate::state::get_session_repository;
use std::collections::HashMap;
use tauri::{command, State};

const DEFAULT_SESSION_LIST_LIMIT: u64 = 100;
const MAX_SESSION_LIST_LIMIT: u64 = 200;

/// Create a new agent session
#[command]
pub async fn agent_create_session(
    manager: State<'_, AgentSessionManager>,
    request: CreateAgentSessionRequest,
) -> Result<SessionMetadata, String> {
    AgentService::create_session(&manager, request).await
}

/// Resume an existing agent session
#[command]
pub async fn agent_resume_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<SessionMetadata, String> {
    manager.resume_session(&session_id).await
}

/// Resume a session and return only the recent transcript slice needed for initial UI rendering.
#[command]
pub async fn agent_open_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    initial_message_limit: Option<u64>,
) -> Result<AgentOpenSessionResponse, String> {
    const DEFAULT_INITIAL_MESSAGE_LIMIT: u64 = 40;

    let session_manager = crate::session::get_session_manager()?;
    crate::session::hydrate_persisted_workspace_override_from_global(session_manager, &session_id)
        .await?;

    let session = manager
        .get_session(&session_id)
        .await?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    let repo = crate::state::get_message_repository();
    let message_slice = repo
        .get_recent_slice(
            &session_id,
            initial_message_limit.unwrap_or(DEFAULT_INITIAL_MESSAGE_LIMIT),
        )
        .await
        .map_err(|e| format!("Failed to load recent session messages: {}", e))?;
    let pending_approvals = {
        let active = manager.active_sessions_arc();
        let sessions = active.read().await;
        if let Some(active_session) = sessions.get(&session_id) {
            let approvals = active_session.pending_approvals.read().await;
            approvals
                .iter()
                .map(|(tool_call_id, data)| PendingApprovalSnapshot {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: data.tool_name.clone(),
                    arguments: data.arguments.clone(),
                    approval_kind: data.approval_kind,
                    request_id: data.request_id.clone(),
                    description: data.description.clone(),
                    input_preview: data.input_preview.clone(),
                })
                .collect()
        } else {
            Vec::new()
        }
    };
    let runtime_state = manager.get_runtime_state(&session_id).await;

    Ok(AgentOpenSessionResponse {
        session,
        messages: message_slice.into(),
        pending_approvals,
        runtime_state,
    })
}

/// Initialize session with messages from database
#[command]
pub async fn agent_init_session_with_messages(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.init_session_with_messages(&session_id).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Session initialized with messages: {}", session_id),
        data: None,
    })
}

/// Create a new session and IMMEDIATELY start the workflow with an initial message
/// This is used for "Draft Mode" where the session is created only when the first message is sent.
#[command]
pub async fn agent_create_session_with_initial_message(
    manager: State<'_, AgentSessionManager>,
    request: CreateAgentSessionWithMessageRequest,
) -> Result<AgentResponse, String> {
    AgentService::create_session_with_initial_message(&manager, request).await
}

/// Update agent configuration for a session
#[command]
pub async fn agent_update_session_config(
    manager: State<'_, AgentSessionManager>,
    request: UpdateAgentConfigRequest,
) -> Result<AgentResponse, String> {
    manager
        .update_session_config(
            request.session_id.clone(),
            request.model,
            request.provider,
            request.agent_config,
        )
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Agent config updated for session: {}", request.session_id),
        data: None,
    })
}

/// Get session metadata
#[command]
pub async fn agent_get_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Option<SessionMetadata>, String> {
    manager.get_session(&session_id).await
}

/// Get all sessions
#[command]
pub async fn agent_get_all_sessions(
    manager: State<'_, AgentSessionManager>,
) -> Result<Vec<SessionMetadata>, String> {
    manager.get_all_sessions().await
}

/// List sessions by latest activity using cursor pagination.
#[command]
pub async fn agent_list_sessions(
    manager: State<'_, AgentSessionManager>,
    request: Option<ListAgentSessionsRequest>,
) -> Result<AgentSessionListResponse, String> {
    let request = request.unwrap_or(ListAgentSessionsRequest {
        cursor: None,
        limit: None,
    });
    let limit = request
        .limit
        .unwrap_or(DEFAULT_SESSION_LIST_LIMIT)
        .clamp(1, MAX_SESSION_LIST_LIMIT);

    manager
        .list_sessions(request.cursor.map(Into::into), limit)
        .await
        .map(Into::into)
}

/// List sessions with unread attention for the notifications UI.
#[command]
pub async fn agent_list_attention_sessions(
    manager: State<'_, AgentSessionManager>,
) -> Result<Vec<SessionMetadata>, String> {
    manager.list_attention_sessions().await
}

/// Call a builtin tool directly via proxy_manager (for testing and direct execution)
/// Returns the unwrapped MCPResult (not the full MCPResponse wrapper)
#[command]
pub async fn agent_call_builtin_tool(
    session_id: String,
    tool_name: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    AgentService::call_builtin_tool(session_id, tool_name, args).await
}

/// Save an attachment to the session-scoped attachment store via an internal UI-only API.
#[command]
pub async fn agent_add_attachment(
    session_id: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    AgentService::add_attachment(session_id, args).await
}

/// Delete an attachment from the session-scoped attachment store via an internal UI-only API.
#[command]
pub async fn agent_delete_attachment(
    session_id: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    AgentService::delete_attachment(session_id, args).await
}

/// Get service contexts for a session
#[command]
pub async fn agent_get_service_contexts(
    session_id: String,
) -> Result<HashMap<String, ServiceContext>, String> {
    AgentService::get_service_contexts(session_id).await
}

/// Delete an agent session and all its data
#[command]
pub async fn agent_delete_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    let deleted_ids = manager.delete_session(session_id.clone()).await?;
    for deleted_id in &deleted_ids {
        remove_lineage(deleted_id).await;
    }

    Ok(AgentResponse {
        success: true,
        message: format!("Session deleted: {}", session_id),
        data: Some(serde_json::json!(deleted_ids)),
    })
}

/// Delete only this session, orphaning its direct children as top-level sessions
#[command]
pub async fn agent_delete_session_only(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    let (deleted_id, orphaned_ids) = manager.delete_session_only(session_id.clone()).await?;
    remove_lineage(&deleted_id).await;

    Ok(AgentResponse {
        success: true,
        message: format!("Session deleted (children orphaned): {}", deleted_id),
        data: Some(serde_json::json!({
            "deletedId": deleted_id,
            "orphanedIds": orphaned_ids
        })),
    })
}

/// Get available tools for a specific agent session
/// Returns the filtered tool list based on agent configuration
/// This ensures UI displays the same tools that LLM can actually use
#[command]
pub async fn agent_get_available_tools(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Vec<MCPTool>, String> {
    manager.get_available_tools(&session_id).await
}

/// Get available tools for a session
#[command]
pub async fn agent_get_tools(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Vec<MCPTool>, String> {
    manager.get_tools_for_session(&session_id).await
}

/// Clear all agent sessions (used for "Clear All Sessions" feature)
#[command]
pub async fn agent_clear_all_sessions(
    manager: State<'_, AgentSessionManager>,
) -> Result<AgentResponse, String> {
    let count = AgentService::clear_all_sessions(&manager).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Cleared {} sessions", count),
        data: None,
    })
}

/// Toggle the bookmark flag on a session
#[command]
pub async fn agent_toggle_session_bookmark(
    session_id: String,
    bookmarked: bool,
) -> Result<(), String> {
    let repo = get_session_repository();
    repo.toggle_bookmark(&session_id, bookmarked)
        .await
        .map_err(|e| format!("Failed to toggle bookmark: {}", e))
}

/// Mark a session as viewed at the current time.
#[command]
pub async fn agent_mark_session_viewed(
    session_id: String,
    viewed_at: Option<i64>,
) -> Result<(), String> {
    let repo = get_session_repository();
    let viewed_at = viewed_at.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    repo.update_last_viewed_at(&session_id, viewed_at)
        .await
        .map_err(|e| format!("Failed to update last viewed timestamp: {}", e))
}

/// Set YOLO mode for a session
#[command]
pub async fn agent_set_yolo_mode(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    enabled: bool,
) -> Result<(), String> {
    manager.set_yolo_mode(&session_id, enabled).await
}

/// Set unsafe mode for a session
#[command]
pub async fn agent_set_unsafe_mode(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    enabled: bool,
) -> Result<(), String> {
    manager.set_unsafe_mode(&session_id, enabled).await
}

/// Set the exclusive execution mode for a session.
#[command]
pub async fn agent_set_execution_mode(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    mode: String,
) -> Result<(), String> {
    manager
        .set_execution_mode(&session_id, mode.parse::<ExecutionMode>()?)
        .await
}

/// Factory reset the agent system (used for "Reset All Data & Settings" feature)
/// Deletes all sessions, assistants, playbooks, mcp servers, and logs.
#[command]
pub async fn agent_factory_reset(
    manager: State<'_, AgentSessionManager>,
) -> Result<AgentResponse, String> {
    AgentService::factory_reset(&manager).await?;

    Ok(AgentResponse {
        success: true,
        message: "Factory reset completed successfully".to_string(),
        data: None,
    })
}
