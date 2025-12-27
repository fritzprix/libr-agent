use crate::agent::AgentSessionManager;

use crate::repositories::SessionMetadata;
use serde::{Deserialize, Serialize};
use tauri::{command, State};

use crate::agent::types::AgentMessageDto;

/// Request to create a new agent session
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSessionRequest {
    pub session_id: String,
    pub name: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
}

/// Request to send a user message to trigger workflow
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageRequest {
    pub session_id: String,
    pub message: AgentMessageDto,
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
    manager
        .create_session(request.session_id, request.name, request.agent_config)
        .await
}

/// Send a user message to start an agent workflow
#[command]
pub async fn agent_send_message(
    manager: State<'_, AgentSessionManager>,
    request: SendUserMessageRequest,
) -> Result<AgentResponse, String> {
    let message = request.message.into_message();
    manager
        .start_workflow(request.session_id.clone(), message)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow started for session: {}", request.session_id),
        data: None,
    })
}

/// Handle LLM response from frontend (called by LLMServiceProvider in TS)
#[command]
pub async fn agent_handle_llm_response(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    assistant_message: AgentMessageDto,
) -> Result<AgentResponse, String> {
    let message = assistant_message.into_message();
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
    manager.resume_workflow(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow resumed for session: {}", session_id),
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

/// Tool execution result from frontend
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionResult {
    pub success: bool,
    pub content: String,
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

/// Handle LLM error from frontend (called by LLMServiceProvider in TS)
#[command]
pub async fn agent_handle_llm_error(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    error: String,
) -> Result<AgentResponse, String> {
    manager.handle_llm_error(session_id.clone(), error).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("LLM error handled for session: {}", session_id),
        data: None,
    })
}

/// Call a builtin tool directly via proxy_manager (for testing and direct execution)
#[command]
pub async fn agent_call_builtin_tool(
    session_id: String,
    tool_name: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use crate::state::get_mcp_service_proxy_manager;

    let proxy_manager = get_mcp_service_proxy_manager();

    let response = proxy_manager
        .call_tool(&session_id, &tool_name, args)
        .await?;

    // Convert MCPResponse to JSON
    serde_json::to_value(response).map_err(|e| format!("Failed to serialize response: {}", e))
}
