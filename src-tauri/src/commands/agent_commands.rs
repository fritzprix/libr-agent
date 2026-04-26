use crate::agent::AgentSessionManager;
use crate::commands::messages_commands::MessageSlice;
use crate::mcp::types::ChannelNotification;
use crate::mcp::types::ChannelPermissionVerdict;
use crate::mcp::types::ServiceContext;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::{CompactContextRecord, SessionMetadata, SessionRepository};
use crate::state::get_session_repository;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{command, AppHandle, State};

use crate::models::chat::Message;
use crate::services::AgentService;
use crate::{
    agent::tools::{create_error_tool_result, create_tool_result_message},
    agent::types::{ToolCall, ToolCallFunction},
};

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

/// Request to inject messages. Workflow continuation is decided by backend
/// session state. `trigger_workflow` is kept only for backward compatibility.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMessagesRequest {
    pub session_id: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub trigger_workflow: bool,
}

/// Request to execute a UI-triggered Tauri action through the backend-owned message path.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteUiTauriActionRequest {
    pub session_id: String,
    pub tool_name: String,
    #[serde(default = "default_ui_action_params")]
    pub params: serde_json::Value,
}

/// Request to inject a channel-originated message into a session.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectChannelMessageRequest {
    pub session_id: String,
    pub server_name: String,
    pub content: String,
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

/// Request to inject a channel-originated message using automatic active-session routing.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectChannelMessageAutoRequest {
    pub server_name: String,
    pub content: String,
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

/// Request to respond to a pending channel permission relay approval.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RespondChannelPermissionRequest {
    pub session_id: String,
    pub request_id: String,
    pub behavior: String,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentOpenSessionResponse {
    pub session: SessionMetadata,
    pub messages: MessageSlice,
    #[serde(default)]
    pub pending_approvals: Vec<PendingApprovalSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingApprovalSnapshot {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: String,
}

fn default_ui_action_params() -> serde_json::Value {
    serde_json::json!({})
}

fn read_required_string(params: &serde_json::Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("UI action parameter '{}' must be a string", key))
}

fn read_optional_string(params: &serde_json::Value, key: &str) -> Result<Option<String>, String> {
    match params.get(key) {
        Some(value) if value.is_null() => Ok(None),
        Some(value) => value
            .as_str()
            .map(|text| Some(text.to_string()))
            .ok_or_else(|| format!("UI action parameter '{}' must be a string", key)),
        None => Ok(None),
    }
}

fn read_required_string_array(
    params: &serde_json::Value,
    key: &str,
) -> Result<Vec<String>, String> {
    let values = params
        .get(key)
        .and_then(|value| value.as_array())
        .ok_or_else(|| format!("UI action parameter '{}' must be an array of strings", key))?;

    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("UI action parameter '{}' must contain only strings", key))
        })
        .collect()
}

fn create_ui_tool_call_message(
    session_id: &str,
    tool_name: &str,
    params: &serde_json::Value,
) -> Result<(String, Message), String> {
    let tool_call_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let arguments = serde_json::to_string(params)
        .map_err(|error| format!("Failed to serialize UI action parameters: {}", error))?;

    Ok((
        tool_call_id.clone(),
        Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: "assistant".to_string(),
            content: Vec::new(),
            tool_calls: Some(vec![ToolCall {
                id: tool_call_id,
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: tool_name.to_string(),
                    arguments,
                },
            }]),
            tool_call_id: None,
            is_streaming: Some(false),
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            created_at: now,
            updated_at: now,
            source: Some("ui".to_string()),
            error: None,
            metadata: None,
        },
    ))
}

async fn execute_ui_tauri_action(
    app_handle: AppHandle,
    request: &ExecuteUiTauriActionRequest,
) -> Result<String, String> {
    match request.tool_name.as_str() {
        "tauri:downloadWorkspaceFile" => {
            crate::commands::download_commands::download_workspace_file(
                app_handle,
                request.session_id.clone(),
                read_required_string(&request.params, "filePath")?,
            )
            .await
        }
        "tauri:downloadMediaFile" => {
            crate::commands::download_commands::download_media_file(
                app_handle,
                Some(request.session_id.clone()),
                read_optional_string(&request.params, "fileName")?,
                read_required_string(&request.params, "mimeType")?,
                read_optional_string(&request.params, "dataBase64")?,
                read_optional_string(&request.params, "fileUrl")?,
            )
            .await
        }
        "tauri:exportAndDownloadZip" => {
            crate::commands::download_commands::export_and_download_zip(
                app_handle,
                request.session_id.clone(),
                read_required_string_array(&request.params, "files")?,
                read_required_string(&request.params, "packageName")?,
            )
            .await
        }
        "tauri:openExternalUrl" => {
            crate::commands::url_commands::open_external_url(read_required_string(
                &request.params,
                "url",
            )?)
            .await?;
            Ok("External URL opened successfully".to_string())
        }
        unsupported => Err(format!("Unsupported UI Tauri action: {}", unsupported)),
    }
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

/// Resume a session and return only the recent transcript slice needed for initial UI rendering.
#[command]
pub async fn agent_open_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    initial_message_limit: Option<u64>,
) -> Result<AgentOpenSessionResponse, String> {
    const DEFAULT_INITIAL_MESSAGE_LIMIT: u64 = 40;

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
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    Ok(AgentOpenSessionResponse {
        session,
        messages: message_slice.into(),
        pending_approvals,
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
    let triggered = manager
        .inject_messages(request.session_id.clone(), request.messages)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Injected messages for session: {} (triggered: {})",
            request.session_id, triggered
        ),
        data: Some(serde_json::json!({ "triggered": triggered })),
    })
}

/// Execute a UI-triggered Tauri action via the backend-owned message lifecycle.
#[command]
pub async fn agent_execute_ui_tauri_action(
    manager: State<'_, AgentSessionManager>,
    app_handle: AppHandle,
    request: ExecuteUiTauriActionRequest,
) -> Result<AgentResponse, String> {
    let (tool_call_id, tool_call_message) =
        create_ui_tool_call_message(&request.session_id, &request.tool_name, &request.params)?;

    let action_result = execute_ui_tauri_action(app_handle, &request).await;

    let (success, result_text, tool_result_message) = match action_result {
        Ok(result_text) => {
            let tool_result_message = create_tool_result_message(
                &request.session_id,
                &tool_call_id,
                result_text.clone(),
                None,
            );
            (true, result_text, tool_result_message)
        }
        Err(error_text) => {
            let tool_result_message =
                create_error_tool_result(&request.session_id, &tool_call_id, &error_text, None);
            (false, error_text, tool_result_message)
        }
    };

    manager
        .inject_messages(
            request.session_id.clone(),
            vec![tool_call_message, tool_result_message],
        )
        .await?;

    Ok(AgentResponse {
        success,
        message: if success {
            format!("UI Tauri action executed: {}", request.tool_name)
        } else {
            format!("UI Tauri action failed: {}", request.tool_name)
        },
        data: Some(serde_json::json!({
            "toolCallId": tool_call_id,
            "result": result_text,
        })),
    })
}

/// Inject a channel-originated message into the session and wake the workflow when idle.
#[command]
pub async fn agent_inject_channel_message(
    manager: State<'_, AgentSessionManager>,
    request: InjectChannelMessageRequest,
) -> Result<AgentResponse, String> {
    let (message_id, triggered) = manager
        .inject_channel_notification(
            request.session_id.clone(),
            request.server_name.clone(),
            ChannelNotification {
                content: request.content,
                meta: request.meta,
            },
        )
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Injected channel message for session: {} from {} ({})",
            request.session_id,
            request.server_name,
            if triggered { "processed" } else { "queued" }
        ),
        data: Some(serde_json::json!({
            "messageId": message_id,
            "status": if triggered { "processed" } else { "queued" }
        })),
    })
}

/// Inject a channel-originated message into the uniquely matching active session for the given
/// channel server and wake the workflow when idle.
#[command]
pub async fn agent_inject_channel_message_auto(
    manager: State<'_, AgentSessionManager>,
    request: InjectChannelMessageAutoRequest,
) -> Result<AgentResponse, String> {
    let (target, message_id, triggered) = manager
        .inject_channel_notification_auto(
            request.server_name.clone(),
            ChannelNotification {
                content: request.content,
                meta: request.meta,
            },
        )
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Injected channel message from {} into session {} ({})",
            request.server_name,
            target.session_name,
            if triggered { "processed" } else { "queued" }
        ),
        data: Some(serde_json::json!({
            "messageId": message_id,
            "sessionId": target.session_id,
            "sessionName": target.session_name,
            "status": if triggered { "processed" } else { "queued" }
        })),
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

    let compaction_pressure = manager
        .handle_llm_response(session_id.clone(), message)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("LLM response processed for session: {}", session_id),
        data: compaction_pressure.map(|pressure| {
            serde_json::json!({
                "compactionPressure": pressure
            })
        }),
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
    pub structured_content: Option<serde_json::Value>,
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

/// Respond to a pending approval using a channel-style request_id and allow/deny behavior.
#[command]
pub async fn agent_respond_channel_permission(
    manager: State<'_, AgentSessionManager>,
    request: RespondChannelPermissionRequest,
) -> Result<AgentResponse, String> {
    let verdict = ChannelPermissionVerdict {
        request_id: request.request_id.clone(),
        behavior: request.behavior.clone(),
    };

    let approved =
        crate::agent::tool_approvals::parse_channel_permission_behavior(&verdict.behavior)?;

    let tool_call_id = manager
        .respond_channel_permission(&request.session_id, &verdict.request_id, approved)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Channel permission responded for request {} (tool_call_id: {}, approved: {})",
            verdict.request_id, tool_call_id, approved
        ),
        data: Some(serde_json::json!({
            "toolCallId": tool_call_id,
            "approved": approved,
        })),
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
    let (deleted_id, orphaned_ids) = manager.delete_session_only(session_id.clone()).await?;

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
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    from_id: String,
    to_id: String,
    summary: String,
) -> Result<AgentResponse, String> {
    manager
        .handle_compact_response(&session_id, from_id, to_id, summary)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Compact response handled for session: {}", session_id),
        data: None,
    })
}

/// Handle a failed compact LLM call — clears the in-flight flag so future turns can retry.
#[command]
pub async fn agent_handle_compact_error(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    error: crate::agent::llm::types::AgentRuntimeError,
) -> Result<AgentResponse, String> {
    manager
        .handle_compact_error(session_id.clone(), error)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Compact error handled for session: {}", session_id),
        data: None,
    })
}
