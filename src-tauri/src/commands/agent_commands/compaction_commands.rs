use super::contracts::AgentResponse;
use crate::agent::AgentSessionManager;
use crate::repositories::CompactContextRecord;
use tauri::{command, State};

/// Get compacted context for a session
#[command]
pub async fn agent_get_compact_context(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Option<CompactContextRecord>, String> {
    manager.get_compact_context(&session_id).await
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
