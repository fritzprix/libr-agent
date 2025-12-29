use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::{MessageRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Start an agent workflow for a session
pub async fn start_workflow(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    user_message: Message,
) -> Result<(), String> {
    // Check if workflow is cancelled before starting
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            if session.cancellation_token.is_cancelled() {
                return Err("Workflow was cancelled before starting".to_string());
            }
        }
    }

    // Update status to Busy
    crate::agent::lifecycle::update_session_status(
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Busy,
    )
    .await?;

    // Emit workflow started event
    let event = crate::agent::events::AgentEvent::WorkflowStarted {
        session_id: session_id.clone(),
    };
    log::info!("Emitting WorkflowStarted event for session: {}", session_id);
    match crate::agent::events::emit_agent_event(app_handle, event) {
        Ok(()) => log::info!("✅ WorkflowStarted event emitted successfully"),
        Err(e) => {
            log::error!("❌ Failed to emit WorkflowStarted event: {}", e);
            return Err(format!("Failed to emit event: {}", e));
        }
    }

    // Ensure cache is initialized before workflow
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    // 1. Add user message to in-memory cache FIRST (immediate, non-blocking)
    {
        let sessions = active_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let mut messages = session.messages.write().await;
        messages.push(user_message.clone());

        // Apply sliding window policy
        if messages.len() > MAX_CACHED_MESSAGES {
            let removed = messages.remove(0);
            log::debug!(
                "Sliding window: evicted oldest message {} from session {}",
                removed.id,
                session_id
            );
        }

        log::info!(
            "📝 Message stack after user message: session={}, count={}, latest_message={}",
            session_id,
            messages.len(),
            user_message.id
        );
    } // Lock released

    // 2. Emit UI event (immediate)
    let message_added_event = crate::agent::events::AgentEvent::MessageAdded {
        session_id: session_id.clone(),
        message: Box::new(user_message.clone()),
    };
    crate::agent::events::emit_agent_event(app_handle, message_added_event)
        .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;

    // 3. Persist to DB asynchronously (fire-and-forget)
    let msg_for_db = user_message.clone();
    let sid_for_db = session_id.clone();
    tokio::spawn(async move {
        let repo = crate::state::get_message_repository();
        if let Err(e) = repo.insert(&msg_for_db).await {
            log::error!(
                "Failed to save user message to DB: session={}, msg_id={}, error={}",
                sid_for_db,
                msg_for_db.id,
                e
            );
        }
    });

    log::info!(
        "Started workflow for session: {} with message: {}",
        session_id,
        user_message.id
    );

    // 4. Request LLM completion with cached messages (no DB query)
    crate::agent::llm::request_llm_completion(
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
    )
    .await?;

    Ok(())
}

/// Pause a running workflow
pub async fn pause_workflow(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    crate::agent::lifecycle::update_session_status(
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Paused,
    )
    .await?;

    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = false;
    }

    log::info!("Paused workflow for session: {}", session_id);
    Ok(())
}

/// Resume a paused workflow
pub async fn resume_workflow(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    crate::agent::lifecycle::update_session_status(
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Busy,
    )
    .await?;

    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = true;
    }

    log::info!("Resumed workflow for session: {}", session_id);
    Ok(())
}

/// Terminate a running workflow
/// This triggers the cancellation token to abort any running operations
pub async fn terminate_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    log::info!("Terminating workflow for session: {}", session_id);

    // Trigger cancellation token to abort running loops
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            session.cancellation_token.cancel();
        } else {
            return Err(format!("Session not found: {}", session_id));
        }
    }

    // Update status to idle (workflow stopped)
    crate::agent::lifecycle::update_session_status(
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Idle,
    )
    .await?;

    // Destroy proxy for this session
    proxy_manager.destroy_proxy(&session_id).await;
    log::info!("Destroyed MCP proxy for session: {}", session_id);

    // Remove from active sessions and create a new cancellation token for future use
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = false;
        // Reset cancellation token for potential future workflows
        session.cancellation_token = CancellationToken::new();
    }

    // Emit workflow stopped event
    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.clone(),
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    log::info!("Terminated workflow for session: {}", session_id);
    Ok(())
}
