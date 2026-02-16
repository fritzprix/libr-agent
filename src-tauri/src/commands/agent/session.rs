use crate::agent::AgentSessionManager;
use crate::repositories::SessionMetadata;
use tauri::{command, State};

use super::types::{
    AgentResponse, CreateAgentSessionRequest, CreateAgentSessionWithMessageRequest,
    UpdateAgentConfigRequest,
};

/// Create a new agent session
#[command]
pub async fn agent_create_session(
    manager: State<'_, AgentSessionManager>,
    request: CreateAgentSessionRequest,
) -> Result<SessionMetadata, String> {
    use crate::repositories::in_memory_session_repository::InMemorySessionRepository;
    use crate::repositories::SessionRepository;
    use std::sync::Arc;

    // Handle workspace override if path is provided
    if let Some(path_str) = &request.workspace_path {
        if let Ok(session_manager) = crate::session::get_session_manager() {
            let path = std::path::PathBuf::from(path_str);
            // Ensure path is absolute and valid
            if path.is_absolute() {
                session_manager
                    .register_session_override(&request.session_id, path)
                    .await?;
            } else {
                return Err("Workspace path must be absolute".to_string());
            }
        } else {
            log::warn!("Failed to get session manager for workspace override");
        }
    }

    // Select repository based on is_ephemeral flag
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
    // 1. Create the session first (persistent by default)
    // We use the default persistent repository here
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

    // 2. Start the workflow with the initial message
    manager
        .start_workflow(request.session_id.clone(), request.message)
        .await
        .map(|_| AgentResponse {
            success: true,
            message: "Session created and workflow started".to_string(),
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

/// Delete an agent session and all its data
#[command]
pub async fn agent_delete_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.delete_session(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Session deleted: {}", session_id),
        data: None,
    })
}
