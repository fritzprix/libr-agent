use crate::agent::types::AgentMessageDto;
use crate::agent::AgentSessionManager;
use tauri::{command, State};

use super::types::{AgentResponse, InjectMessagesRequest, SendUserMessageRequest};

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
    assistant_message: AgentMessageDto,
) -> Result<AgentResponse, String> {
    // AgentMessageDto is now a type alias for Message, no conversion needed
    let message = assistant_message;

    log::info!(
        "📥 Received LLM response from frontend: session={}, message_id={}, has_tool_calls={}, tool_call_count={}, content_len={}",
        session_id,
        message.id,
        message.tool_calls.is_some(),
        message.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0),
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
