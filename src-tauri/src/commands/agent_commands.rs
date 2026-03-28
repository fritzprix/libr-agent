use crate::agent::AgentSessionManager;
use crate::mcp::types::ServiceContext;
use crate::repositories::{CompactContextRecord, SessionMetadata, SessionRepository};
use crate::state::get_session_repository;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{command, AppHandle, Emitter, State};

use crate::models::chat::Message;
use crate::services::AgentService;

/// Request to create a new agent session
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSessionRequest {
    pub session_id: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
    #[serde(default)]
    pub is_ephemeral: bool,
    pub workspace_path: Option<String>,
}

/// Request to create a new session and send the first message in one go
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSessionWithMessageRequest {
    pub session_id: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
    pub message: Message,
    pub workspace_path: Option<String>,
}

/// Request to send a user message to trigger workflow
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageRequest {
    pub session_id: String,
    pub message: Message,
}

/// Request to inject messages silently or with workflow trigger
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMessagesRequest {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub trigger_workflow: bool,
}

/// Request to update agent configuration for a session
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentConfigRequest {
    pub session_id: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
}

/// Response for agent operations
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

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

/// Send a user message to start an agent workflow
#[command]
pub async fn agent_send_message(
    manager: State<'_, AgentSessionManager>,
    request: SendUserMessageRequest,
) -> Result<AgentResponse, String> {
    // Message is already the correct type, no conversion needed
    let message = request.message;

    manager
        .start_workflow(request.session_id, message)
        .await
        .map(|_| AgentResponse {
            success: true,
            message: "Message sent".to_string(),
            data: None,
        })
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

/// Inject messages into the session
#[command]
pub async fn agent_inject_messages(
    manager: State<'_, AgentSessionManager>,
    request: InjectMessagesRequest,
) -> Result<AgentResponse, String> {
    manager
        .inject_messages(
            request.session_id.clone(),
            request.messages,
            request.trigger_workflow,
        )
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Injected messages for session: {} (triggered: {})",
            request.session_id, request.trigger_workflow
        ),
        data: None,
    })
}

/// Handle LLM response from frontend (called by LLMServiceProvider in TS)
#[command]
pub async fn agent_handle_llm_response(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    assistant_message: Message,
) -> Result<AgentResponse, String> {
    // AgentMessageDto was a type alias for Message, no conversion needed
    // Message is the direct type (AgentMessageDto was a deprecated alias for Message)
    let message = assistant_message;

    log::info!(
        "📥 Received LLM response from frontend: session={}, message_id={}, has_tool_calls={}, tool_call_count={}, content_len={}",
        session_id,
        message.id,
        message.tool_calls.is_some(),
        message.tool_calls.as_ref().map(|tc: &Vec<crate::agent::types::ToolCall>| tc.len()).unwrap_or(0),
        message.content.len()
    );

    log::debug!("📥 Full message received: {:#?}", message);

    manager
        .handle_llm_response(session_id.clone(), message)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("LLM response processed for session: {}", session_id),
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

/// Pause a running workflow
#[command]
pub async fn agent_pause_workflow(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.pause_workflow(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow paused for session: {}", session_id),
        data: None,
    })
}

/// Resume a paused workflow
#[command]
pub async fn agent_resume_workflow(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    // Resume the workflow (internal logic handles cache validation)
    manager.resume_workflow(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow resumed: {}", session_id),
        data: None,
    })
}

/// Terminate a running workflow
#[command]
pub async fn agent_terminate_workflow(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.terminate_session(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow terminated for session: {}", session_id),
        data: None,
    })
}

/// Cancel a running workflow
#[command]
pub async fn agent_cancel_workflow(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.cancel_workflow(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow cancel requested for session: {}", session_id),
        data: None,
    })
}

/// Tool execution result from frontend
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionResult {
    pub success: bool,
    pub content: String,
    pub mcp_content: Option<Vec<crate::mcp::types::MCPContent>>,
    pub error: Option<String>,
    pub is_error: bool,
}

/// Handle tool execution result from frontend (called by ToolBridgeProvider in TS)
#[command]
pub async fn agent_handle_tool_result(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    tool_call_id: String,
    result: ToolExecutionResult,
) -> Result<AgentResponse, String> {
    manager
        .handle_tool_result(session_id.clone(), tool_call_id, result)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Tool result processed for session: {}", session_id),
        data: None,
    })
}

/// Respond to a pending tool execution approval
#[command]
pub async fn agent_respond_tool_approval(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    tool_call_id: String,
    approved: bool,
) -> Result<AgentResponse, String> {
    manager
        .respond_tool_approval(&session_id, &tool_call_id, approved)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Tool approval responded for {}: {}", tool_call_id, approved),
        data: None,
    })
}

/// Handle LLM error from frontend (called by LLMServiceProvider in TS)
#[command]
pub async fn agent_handle_llm_error(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    error: crate::agent::llm::types::AgentRuntimeError,
) -> Result<AgentResponse, String> {
    manager.handle_llm_error(session_id.clone(), error).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("LLM error handled for session: {}", session_id),
        data: None,
    })
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
    manager.delete_session_only(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Session deleted (children orphaned): {}", session_id),
        data: None,
    })
}

/// Get available tools for a specific agent session
/// Returns the filtered tool list based on agent configuration
/// This ensures UI displays the same tools that LLM can actually use
#[command]
pub async fn agent_get_available_tools(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    manager.get_available_tools(&session_id).await
}

/// Get available tools for a session
#[command]
pub async fn agent_get_tools(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
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

/// Get compacted context for a session
#[command]
pub async fn agent_get_compact_context(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Option<CompactContextRecord>, String> {
    manager.get_compact_context(&session_id).await
}

/// Save compacted context for a session
#[command]
pub async fn agent_save_compact_context(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    record: CompactContextRecord,
) -> Result<AgentResponse, String> {
    manager.save_compact_context(&session_id, record).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Compact context saved for session: {}", session_id),
        data: None,
    })
}

/// Handle a successful compact response from the frontend LLM call.
/// Stores the summary in-memory + DB and clears the in-flight flag.
#[command]
pub async fn agent_handle_compact_response(
    app_handle: AppHandle,
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    from_id: String,
    to_id: String,
    summary: String,
) -> Result<AgentResponse, String> {
    manager
        .handle_compact_response(&session_id, from_id, to_id, summary)
        .await?;
    let session_name = manager.get_session_display_name(&session_id).await;

    let state_event = crate::agent::llm::types::CompactStateEvent {
        session_id: session_id.clone(),
        session_name,
        compacting: false,
        phase: crate::agent::llm::types::CompactStatePhase::Succeeded,
    };
    if let Err(e) = app_handle.emit("llm:compact-state", state_event) {
        log::error!("Failed to emit llm:compact-state (done): {}", e);
    }

    Ok(AgentResponse {
        success: true,
        message: format!("Compact response handled for session: {}", session_id),
        data: None,
    })
}

/// Handle a failed compact LLM call — clears the in-flight flag so future turns can retry.
#[command]
pub async fn agent_handle_compact_error(
    app_handle: AppHandle,
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.clear_compact_in_flight(&session_id).await;
    let session_name = manager.get_session_display_name(&session_id).await;
    log::warn!(
        "⚠️ Compact error received for session {}, flag cleared",
        session_id
    );

    let state_event = crate::agent::llm::types::CompactStateEvent {
        session_id: session_id.clone(),
        session_name,
        compacting: false,
        phase: crate::agent::llm::types::CompactStatePhase::Failed,
    };
    if let Err(e) = app_handle.emit("llm:compact-state", state_event) {
        log::error!("Failed to emit llm:compact-state (error): {}", e);
    }

    Ok(AgentResponse {
        success: true,
        message: format!("Compact error cleared for session: {}", session_id),
        data: None,
    })
}
