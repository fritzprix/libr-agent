use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{MessageRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Start an agent workflow for a session
pub async fn start_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    user_message: Message,
) -> Result<(), String> {
    // Check status, deduplicate, and queue if busy (Atomic Check-and-Act)
    let should_queue = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            // Note: we intentionally do NOT check is_cancelled() here.
            // cancel_workflow (soft cancel) leaves the token in a cancelled state to block
            // stale LLM responses, but start_workflow resets it unconditionally below.
            // Failing here would prevent users from sending new messages after a cancel.

            // Deduplicate: Check if message ID already exists
            {
                let messages = session.messages.read().await;
                if messages.iter().any(|m| m.id == user_message.id) {
                    log::warn!(
                        "Ignoring duplicate message start_workflow: {}",
                        user_message.id
                    );
                    return Ok(());
                }
            }

            // Check Status
            if session.metadata.status == SessionStatus::Busy {
                log::info!(
                    "Session {} is busy. Queueing message: {} for next cycle.",
                    session_id,
                    user_message.id
                );

                // 1. Add to in-memory cache (while holding session lock)
                {
                    let mut messages = session.messages.write().await;
                    messages.push(user_message.clone());
                    if messages.len() > MAX_CACHED_MESSAGES {
                        messages.remove(0);
                    }
                }

                // 2. Add to Pending Events (while holding session lock)
                {
                    let mut pending = session.pending_events.write().await;
                    pending.add(crate::agent::state::PendingEvent::Message(
                        user_message.id.clone(),
                    ));
                }

                true // Signal that we queued it
            } else {
                false // Not busy, proceed to start workflow
            }
        } else {
            false // Session not found, will be handled by standard flow (or fail there)
        }
    }; // Lock released here

    if should_queue {
        // 3. Persist to DB (Async I/O outside lock)
        let repo = crate::state::get_message_repository();
        if let Err(e) = repo.insert(&user_message).await {
            log::error!("Failed to save queued user message to DB: {}", e);
        }

        // Do NOT call request_llm_completion. The existing busy workflow will pick it up.
        // Do NOT emit MessageAdded (it will be emitted when drained).
        return Ok(());
    }

    // Explicit new workflow start: reset all cancellation state.
    // Uses a write lock to also reset the cancellation_token, which cancel_workflow
    // may have left in a cancelled state to block stale LLM responses.
    {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            session.cancel_pending.store(false, Ordering::SeqCst);
            session.cancellation_token = CancellationToken::new();
        }
    }

    // --- STANDARD START WORKFLOW (Idle/Paused) ---

    // Update status to Busy
    crate::agent::lifecycle::update_session_status(
        session_repo,
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
        Ok(()) => log::info!("??WorkflowStarted event emitted successfully"),
        Err(e) => {
            log::error!("??Failed to emit WorkflowStarted event: {}", e);
            return Err(format!("Failed to emit event: {}", e));
        }
    }

    // Ensure cache is initialized before workflow
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    // 1. Add user message to in-memory cache FIRST (immediate, non-blocking)
    {
        let sessions = active_sessions.read().await;
        // Logic duplicated for Idle path but that's fine for clarity vs refactoring whole function
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let mut messages = session.messages.write().await;
        messages.push(user_message.clone());

        // Apply sliding window policy
        if messages.len() > MAX_CACHED_MESSAGES {
            let removed = messages.remove(0);
            log::debug!("Sliding window evicted: {}", removed.id);
        }

        log::info!(
            "?뱷 Message stack after user message: session={}, count={}, latest_message={}",
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

    // 3. Persist to DB synchronously to ensure data integrity
    let repo = crate::state::get_message_repository();
    if let Err(e) = repo.insert(&user_message).await {
        log::error!(
            "Failed to save user message to DB: session={}, msg_id={}, error={}",
            session_id,
            user_message.id,
            e
        );
        return Err(format!("Failed to persist message: {}", e));
    }

    log::info!(
        "Started workflow for session: {} with message: {}",
        session_id,
        user_message.id
    );

    // 4. Ensure Proxy Exists (Critical for System Prompt)
    if proxy_manager.get_proxy(&session_id).await.is_none() {
        log::warn!(
            "MCP proxy missing for session {} during workflow start. Recreating...",
            session_id
        );

        // 4.1 Get session metadata to retrieve config
        let session_repo = crate::state::get_session_repository();
        if let Some(session) = session_repo
            .get_session(&session_id)
            .await
            .map_err(|e| format!("Failed to get session for proxy recreation: {}", e))?
        {
            // 4.2 Parse agent config
            if let Some(config_json) = session.agent_config {
                let agent_config = crate::agent::AgentConfig::from_json(&config_json)
                    .map_err(|e| format!("Failed to parse agent config: {}", e))?;

                // 4.3 Extract tool IDs
                let tool_ids = crate::agent::tools::extract_builtin_tool_ids(&agent_config);
                let mcp_server_ids = agent_config.mcp_server_ids.clone();

                // 4.4 Recreate proxy
                proxy_manager
                    .create_proxy(
                        session_id.clone(),
                        tool_ids,
                        mcp_server_ids,
                        Some(app_handle.clone()),
                    )
                    .await?;

                log::info!(
                    "??Successfully recreated MCP proxy for session: {}",
                    session_id
                );
            } else {
                log::error!(
                    "Cannot recreate proxy: Session {} has no agent config",
                    session_id
                );
            }
        } else {
            log::error!(
                "Cannot recreate proxy: Session {} not found in DB",
                session_id
            );
        }
    }

    // 5. Request LLM completion with cached messages (no DB query)
    crate::agent::llm::request_llm_completion(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
    )
    .await?;

    Ok(())
}
