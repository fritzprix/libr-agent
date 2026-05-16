use super::AgentSessionManager;
use crate::models::chat::Message;
use crate::repositories::SessionStatus;

pub async fn inject_messages(
    manager: &AgentSessionManager,
    session_id: String,
    messages: Vec<Message>,
) -> Result<bool, String> {
    let should_trigger_workflow = {
        let sessions = manager.active_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        let is_transitioning_to_busy = matches!(
            session.status_transition.read().await.as_ref(),
            Some(crate::agent::state::SessionStatusTransition::ToStatus(
                SessionStatus::Busy
            ))
        );

        session.metadata.status != SessionStatus::Busy && !is_transitioning_to_busy
    };

    // Delegate message persistence, caching, and event emission to MessageService
    crate::services::MessageService::inject_messages_to_session(
        &manager.active_sessions,
        &manager.app_handle,
        &session_id,
        messages,
        should_trigger_workflow,
    )
    .await?;

    if should_trigger_workflow {
        log::info!(
            "Triggering workflow after message injection for session: {}",
            session_id
        );

        {
            let mut sessions = manager.active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                crate::agent::workflow::start::reset_session_execution_state(session).await;
            }
        }

        // Update status to Busy
        crate::agent::lifecycle::update_session_status(
            &manager.session_repo,
            &manager.active_sessions,
            &manager.app_handle,
            &session_id,
            crate::repositories::SessionStatus::Busy,
        )
        .await?;

        // Emit workflow started event
        let event = crate::agent::events::AgentEvent::WorkflowStarted {
            session_id: session_id.clone(),
        };
        if let Err(e) = crate::agent::tauri_events::emit_agent_event(&manager.app_handle, event) {
            log::error!(
                "Failed to emit WorkflowStarted event during injection: {}",
                e
            );
        }

        crate::agent::workflow::start::ensure_proxy_ready(
            &manager.proxy_manager,
            &manager.app_handle,
            &session_id,
            60,
        )
        .await?;

        crate::agent::llm::request_llm_completion_with_recovery(
            &manager.session_repo,
            &manager.active_sessions,
            &manager.proxy_manager,
            &manager.app_handle,
            session_id,
        )
        .await?;
    }

    Ok(should_trigger_workflow)
}
