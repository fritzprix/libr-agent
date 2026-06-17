use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
/// Pause a running workflow
pub async fn pause_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Paused,
    )
    .await?;

    log::info!("Paused workflow for session: {}", session_id);
    Ok(())
}

/// Resume a paused workflow
pub async fn resume_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            crate::agent::workflow::start::reset_session_execution_state(session).await;
        }
    }

    // Ensure cache is initialized before resuming (lazy load if needed, preserve if exists)
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    // Transition status to Queued immediately
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Queued,
    )
    .await?;

    log::info!("Queued workflow resume for session: {}", session_id);

    let session_repo = Arc::clone(session_repo);
    let active_sessions = Arc::clone(active_sessions);
    let proxy_manager = Arc::clone(proxy_manager);
    let app_handle = app_handle.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        // 1. Acquire active agent permit first (blocks safely outside transition lock)
        let gate = crate::state::get_concurrency_gate();
        let permit = match gate.acquire_active_agent().await {
            Ok(p) => p,
            Err(e) => {
                log::error!(
                    "Failed to acquire active agent permit for session {}: {}",
                    session_id_clone,
                    e
                );
                let error_event = crate::agent::events::AgentEvent::WorkflowError {
                    session_id: session_id_clone.clone(),
                    error: crate::agent::llm::types::AgentRuntimeError::new(
                        crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                        e.to_string(),
                    )
                    .with_code("BACKGROUND_RESUME_FAILED"),
                };
                let _ = crate::agent::tauri_events::emit_agent_event(&app_handle, error_event);
                return;
            }
        };

        // 2. Put permit into memory and verify not cancelled/changed
        {
            let mut active = active_sessions.write().await;
            if let Some(session) = active.get_mut(&session_id_clone) {
                if session.cancellation_token.is_cancelled()
                    || session.metadata.status != SessionStatus::Queued
                {
                    log::info!(
                        "Session {} cancelled or status changed while queued. Aborting background resume.",
                        session_id_clone
                    );
                    drop(permit);
                    return;
                }
                session.active_permit = Some(permit);
            } else {
                log::warn!(
                    "Session {} not found in active sessions during background resume. Aborting.",
                    session_id_clone
                );
                drop(permit);
                return;
            }
        }

        // Transition status to Busy (bypasses concurrency gate block since permit is held)
        if let Err(e) = crate::agent::lifecycle::update_session_status(
            &session_repo,
            &active_sessions,
            &app_handle,
            &session_id_clone,
            SessionStatus::Busy,
        )
        .await
        {
            log::error!(
                "Failed to transition resumed session {} to Busy: {}",
                session_id_clone,
                e
            );
            {
                let mut active = active_sessions.write().await;
                if let Some(session) = active.get_mut(&session_id_clone) {
                    session.active_permit.take();
                }
            }
            let error_event = crate::agent::events::AgentEvent::WorkflowError {
                session_id: session_id_clone.clone(),
                error: crate::agent::llm::types::AgentRuntimeError::new(
                    crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                    e.to_string(),
                )
                .with_code("BACKGROUND_RESUME_FAILED"),
            };
            let _ = crate::agent::tauri_events::emit_agent_event(&app_handle, error_event);
            return;
        }

        // Ensure proxy is ready
        if let Err(e) = crate::agent::workflow::start::ensure_proxy_ready(
            &proxy_manager,
            &app_handle,
            &session_id_clone,
            60,
        )
        .await
        {
            log::error!(
                "Proxy check failed during background resume for session {}: {}",
                session_id_clone,
                e
            );
            let _ = crate::agent::lifecycle::update_session_status(
                &session_repo,
                &active_sessions,
                &app_handle,
                &session_id_clone,
                SessionStatus::Error,
            )
            .await;
            let error_event = crate::agent::events::AgentEvent::WorkflowError {
                session_id: session_id_clone.clone(),
                error: crate::agent::llm::types::AgentRuntimeError::new(
                    crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                    e.to_string(),
                )
                .with_code("BACKGROUND_RESUME_FAILED"),
            };
            let _ = crate::agent::tauri_events::emit_agent_event(&app_handle, error_event);
            return;
        }

        // Trigger LLM to pick up where it left off
        if let Err(e) = crate::agent::llm::request_llm_completion_with_recovery(
            &session_repo,
            &active_sessions,
            &proxy_manager,
            &app_handle,
            session_id_clone.clone(),
        )
        .await
        {
            log::error!(
                "LLM completion failed in background resume for session {}: {:?}",
                session_id_clone,
                e
            );
            let _ = crate::agent::lifecycle::update_session_status(
                &session_repo,
                &active_sessions,
                &app_handle,
                &session_id_clone,
                SessionStatus::Error,
            )
            .await;
            let error_event = crate::agent::events::AgentEvent::WorkflowError {
                session_id: session_id_clone.clone(),
                error: crate::agent::llm::types::AgentRuntimeError::new(
                    crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                    e.to_string(),
                )
                .with_code("BACKGROUND_RESUME_FAILED"),
            };
            let _ = crate::agent::tauri_events::emit_agent_event(&app_handle, error_event);
        }
    });

    Ok(())
}
