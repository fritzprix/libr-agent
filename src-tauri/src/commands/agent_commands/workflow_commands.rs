use super::contracts::{
    AgentResponse, InjectMessagesRequest, SendUserMessageRequest, ToolExecutionResult,
};
use crate::agent::AgentSessionManager;
use crate::models::chat::Message;
use crate::services::agent_service::remove_lineage;
use tauri::{command, State};

/// Send a user message to start an agent workflow
///
/// MCP external-server discovery must not block session start. The workflow
/// queues the message immediately and waits for proxy readiness in the
/// background (`ensure_proxy_ready`). Partial/failed MCP servers surface via
/// runtime-state events (Sonner toasts) instead of rejecting this command.
#[command]
pub async fn agent_send_message(
    manager: State<'_, AgentSessionManager>,
    request: SendUserMessageRequest,
) -> Result<AgentResponse, String> {
    manager
        .start_workflow(request.session_id, request.message)
        .await
        .map(|_| AgentResponse {
            success: true,
            message: "Message sent".to_string(),
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

/// List durable waiting prompts for a session (FIFO).
#[command]
pub async fn agent_get_pending_queue(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    let messages = manager.get_pending_queue(&session_id).await?;
    Ok(AgentResponse {
        success: true,
        message: format!("Pending queue for session: {}", session_id),
        data: Some(serde_json::to_value(messages).map_err(|e| e.to_string())?),
    })
}

/// Cancel a single waiting prompt without aborting the active turn.
#[command]
pub async fn agent_cancel_pending_prompt(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    message_id: String,
) -> Result<AgentResponse, String> {
    let removed = manager
        .cancel_pending_prompt(&session_id, &message_id)
        .await?;
    Ok(AgentResponse {
        success: true,
        message: if removed {
            format!("Cancelled pending prompt {}", message_id)
        } else {
            format!("Pending prompt {} not found", message_id)
        },
        data: Some(serde_json::json!({ "removed": removed })),
    })
}

/// Handle LLM response from frontend (called by LLMServiceProvider in TS)
#[command]
pub async fn agent_handle_llm_response(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    assistant_message: Message,
) -> Result<AgentResponse, String> {
    log::info!(
        "📥 Received LLM response from frontend: session={}, message_id={}, has_tool_calls={}, tool_call_count={}, content_len={}",
        session_id,
        assistant_message.id,
        assistant_message.tool_calls.is_some(),
        assistant_message
            .tool_calls
            .as_ref()
            .map(|tool_calls| tool_calls.len())
            .unwrap_or(0),
        assistant_message.content.len()
    );

    log::debug!("📥 Full message received: {:#?}", assistant_message);

    manager
        .handle_llm_response(session_id.clone(), assistant_message)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("LLM response processed for session: {}", session_id),
        data: None,
    })
}

#[command]
pub async fn agent_report_llm_streaming_issue(
    manager: State<'_, AgentSessionManager>,
    report: crate::agent::llm::types::StreamingIssueReport,
) -> Result<AgentResponse, String> {
    let outcome = manager.report_llm_streaming_issue(report.clone()).await?;
    let (message, action) = match outcome {
        crate::agent::llm::StreamingIssueOutcome::Ignored => (
            format!(
                "Ignored stale streaming issue for session {}",
                report.session_id
            ),
            "ignored",
        ),
        crate::agent::llm::StreamingIssueOutcome::Retried { retry_count } => {
            let (label, max_retries) = match report.issue_kind {
                crate::agent::llm::types::StreamingIssueKind::RepeatedThinkingLoop => (
                    "repeated thinking loop",
                    crate::agent::llm::REPEATED_THINKING_MAX_RETRIES,
                ),
                crate::agent::llm::types::StreamingIssueKind::RepeatedTextLoop => (
                    "repeated text loop",
                    crate::agent::llm::REPEATED_TEXT_LOOP_MAX_RETRIES,
                ),
            };
            (
                format!(
                    "Retried completion after {label} for session {} (retry {}/{max_retries})",
                    report.session_id, retry_count,
                ),
                "retried",
            )
        }
        crate::agent::llm::StreamingIssueOutcome::Failed => {
            let label = match report.issue_kind {
                crate::agent::llm::types::StreamingIssueKind::RepeatedThinkingLoop => {
                    "repeated thinking loop"
                }
                crate::agent::llm::types::StreamingIssueKind::RepeatedTextLoop => {
                    "repeated text loop"
                }
            };
            (
                format!(
                    "Stopped workflow after {label} for session {}",
                    report.session_id
                ),
                "failed",
            )
        }
        crate::agent::llm::StreamingIssueOutcome::FallThrough => (
            format!(
                "Recovery budget exhausted for session {}; continuing with original completion",
                report.session_id
            ),
            "fall_through",
        ),
    };

    Ok(AgentResponse {
        success: true,
        message,
        data: Some(serde_json::json!({
            "action": action,
        })),
    })
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
    remove_lineage(&session_id).await;

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
