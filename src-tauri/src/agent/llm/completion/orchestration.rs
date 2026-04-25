use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub async fn request_llm_completion_with_recovery(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    match super::request::request_llm_completion(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id.clone(),
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(error) => {
            let outcome = crate::agent::llm::handle_llm_error_with_outcome(
                session_repo,
                active_sessions,
                app_handle,
                session_id,
                error.clone(),
            )
            .await?;
            crate::agent::llm::completion_result_from_error_handling_outcome(outcome, error)
        }
    }
}
