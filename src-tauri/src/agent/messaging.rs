use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES, PendingEvent};
use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Inject messages into the session and optionally trigger the workflow
pub async fn inject_messages(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    messages: Vec<Message>,
    trigger_workflow: bool,
) -> Result<(), String> {
    // 1. Ensure cache is initialized
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id)
        .await?;

    // 2. Get session reference (single lock acquisition)
    let sessions = active_sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // 3. Add messages to in-memory cache
    {
        let mut session_messages = session.messages.write().await;
        for msg in &messages {
            session_messages.push(msg.clone());
            if session_messages.len() > MAX_CACHED_MESSAGES {
                session_messages.remove(0);
            }
        }
    }

    // 4. Emit MessageAdded events ONLY when triggering workflow
    // When triggerWorkflow=false, messages stay in backend cache without UI update
    // Frontend will add to pendingMessages queue and display with pending state
    if trigger_workflow {
        // Drop session lock before I/O operations
        drop(sessions);

        for msg in &messages {
            let event = crate::agent::events::AgentEvent::MessageAdded {
                session_id: session_id.clone(),
                message: Box::new(msg.clone()),
            };
            crate::agent::events::emit_agent_event(app_handle, event)
                .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;
        }
    } else {
        // Track these message IDs as pending (will emit when workflow picks them up)
        let mut pending_events = session.pending_events.write().await;
        for msg in &messages {
            pending_events.add(PendingEvent::Message(msg.id.clone()));
        }
        log::info!(
            "Marked {} messages as pending for session: {} (IDs: {:?})",
            messages.len(),
            session_id,
            messages.iter().map(|m| &m.id).collect::<Vec<_>>()
        );
    }

    // 5. Persist to DB asynchronously
    let msgs_for_db = messages.clone();
    tokio::spawn(async move {
        use crate::repositories::message_repository::MessageRepository;
        let repo = crate::state::get_message_repository();
        for msg in msgs_for_db {
            if let Err(e) = repo.insert(&msg).await {
                log::error!("Failed to inject message to DB: {}", e);
            }
        }
    });

    // 5. Trigger workflow if requested
    if trigger_workflow {
        log::info!(
            "Triggering workflow after message injection for session: {}",
            session_id
        );

        {
            let sessions = active_sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                session.cancel_pending.store(false, Ordering::SeqCst);
            }
        }

        // [Fix Option 1] Inline status update to ensure UI reflects 'Busy' state
        // 1. Update status to Busy
        crate::agent::lifecycle::update_session_status(
            session_repo,
            active_sessions,
            app_handle,
            &session_id,
            crate::repositories::SessionStatus::Busy,
        )
        .await?;

        // 2. Emit workflow started event
        let event = crate::agent::events::AgentEvent::WorkflowStarted {
            session_id: session_id.clone(),
        };
        if let Err(e) = crate::agent::events::emit_agent_event(app_handle, event) {
            log::error!(
                "Failed to emit WorkflowStarted event during injection: {}",
                e
            );
        }

        // We use request_llm_completion directly here as we don't need the full start_workflow logic
        // (which assumes a User message as input)
        crate::agent::llm::request_llm_completion(
            session_repo,
            active_sessions,
            proxy_manager,
            app_handle,
            session_id,
        )
        .await?;
    }

    Ok(())
}

/// Remove a message from the in-memory cache
/// Used when messages are deleted via messages_delete command to keep cache in sync
pub async fn remove_message_from_cache(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message_id: &str,
) -> Result<(), String> {
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut messages = session.messages.write().await;
        messages.retain(|m| m.id != message_id);
        log::debug!(
            "Removed message {} from in-memory cache for session {}. Remaining: {}",
            message_id,
            session_id,
            messages.len()
        );
        Ok(())
    } else {
        // Session not active in memory - no cache to update (this is OK)
        log::debug!(
            "Session {} not active, skipping in-memory cache update for message deletion",
            session_id
        );
        Ok(())
    }
}
