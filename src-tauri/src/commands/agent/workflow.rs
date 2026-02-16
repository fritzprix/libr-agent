use crate::agent::AgentSessionManager;
use tauri::{command, State};

use super::types::AgentResponse;

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
