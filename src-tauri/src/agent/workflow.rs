use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{MessageRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelStrategy {
    DeferToMessageBoundary,
    StopImmediately,
}

pub fn classify_cancel_strategy(has_pending_execution: bool) -> CancelStrategy {
    if has_pending_execution {
        CancelStrategy::DeferToMessageBoundary
    } else {
        CancelStrategy::StopImmediately
    }
}

pub fn should_consume_cancel_at_message_boundary(cancel_pending: bool) -> bool {
    cancel_pending
}

/// Start an agent workflow for a session
pub async fn start_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    user_message: Message,
) -> Result<(), String> {
    // Wait for background tool loading to complete before starting the LLM workflow.
    // Prevents the agent from starting with an empty tool list when external MCP servers
    // are still being discovered (spawned asynchronously inside create_proxy).
    if let Err(e) = proxy_manager.wait_until_proxy_ready(&session_id, 60).await {
        log::warn!(
            "Tool readiness wait failed for session {}: {}. Proceeding anyway.",
            session_id,
            e
        );
    }

    // Ensure the message cache is populated from DB before the dedup check.
    // Without this, an uninitialized (empty) cache would silently pass the duplicate
    // check, and the session would get stuck in Busy state when the second dedup
    // in append_user_message_to_session correctly rejects the duplicate after init.
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    // Note: we intentionally do NOT check is_cancelled() here.
    // cancel_workflow (soft cancel) leaves the token in a cancelled state to block
    // stale LLM responses, but start_workflow resets it unconditionally below.
    // Failing here would prevent users from sending new messages after a cancel.

    // Check status, deduplicate, and queue if busy (Atomic Check-and-Act)
    let should_queue = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
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
                    "Session {} is busy. Queueing message: {} in pending_events only.",
                    session_id,
                    user_message.id
                );
                true // Signal that we queued it
            } else {
                false // Not busy, proceed to start workflow
            }
        } else {
            false // Session not found, will be handled by standard flow (or fail there)
        }
    }; // Lock released here

    if should_queue {
        // Add to Pending Events and Persist to DB without touching session.messages
        // This prevents polluting the active context window mid-tool-execution
        crate::services::MessageService::queue_user_message(
            active_sessions,
            &session_id,
            &user_message,
        )
        .await?;

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
            // Safety valve: clear any stale in-flight compaction flag.
            // Guards against the case where the frontend crashed mid-compaction
            // and never called agent_handle_compact_error to release the flag.
            session.compact_in_flight.store(false, Ordering::SeqCst);
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
        Ok(()) => log::info!("✅ WorkflowStarted event emitted successfully"),
        Err(e) => {
            log::error!("❌ Failed to emit WorkflowStarted event: {}", e);
            return Err(format!("Failed to emit event: {}", e));
        }
    }

    // Delegate message deduplication, cache update, DB insertion, and UI event emission
    // to the MessageService to maintain clean architectural boundaries.
    crate::services::MessageService::append_user_message_to_session(
        active_sessions,
        app_handle,
        &session_id,
        &user_message,
    )
    .await?;

    log::info!(
        "Started workflow for session: {} with message: {}",
        session_id,
        user_message.id
    );

    // Ensure Proxy Exists (Critical for System Prompt)
    ensure_proxy_exists(proxy_manager, app_handle, &session_id).await?;

    // Request LLM completion with cached messages (no DB query)
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

/// Helper to ensure a proxy exists for the session before invoking LLM
async fn ensure_proxy_exists(
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<(), String> {
    if proxy_manager.get_proxy(session_id).await.is_some() {
        return Ok(());
    }

    log::warn!(
        "MCP proxy missing for session {} during workflow start. Recreating...",
        session_id
    );

    let session_repo = crate::state::get_session_repository();
    if let Some(session) = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to get session for proxy recreation: {}", e))?
    {
        if let Some(config_json) = session.agent_config {
            let agent_config = crate::agent::AgentConfig::from_json(&config_json)
                .map_err(|e| format!("Failed to parse agent config: {}", e))?;

            let tool_ids = crate::agent::tools::extract_builtin_tool_ids(&agent_config);
            let mcp_server_ids = agent_config.mcp_server_ids.clone();

            proxy_manager
                .create_proxy(
                    session_id.to_string(),
                    tool_ids,
                    mcp_server_ids,
                    Some(app_handle.clone()),
                )
                .await?;

            log::info!(
                "✅ Successfully recreated MCP proxy for session: {}",
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

    Ok(())
}

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

    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = false;
    }

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
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            session.cancel_pending.store(false, Ordering::SeqCst);
        }
    }

    // Ensure cache is initialized before resuming (lazy load if needed, preserve if exists)
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    crate::agent::lifecycle::update_session_status(
        session_repo,
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
    drop(active); // Drop lock before async call

    log::info!("Resumed workflow status for session: {}", session_id);

    // Trigger LLM to pick up where it left off
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

/// Terminate a running workflow
/// This triggers the cancellation token to abort any running operations
pub async fn terminate_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    log::info!("Terminating workflow for session: {}", session_id);

    // 1. Trigger cancellation token if the session is active in memory
    let session_active = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            session.cancel_pending.store(true, Ordering::SeqCst);
            session.cancellation_token.cancel();
            true
        } else {
            false
        }
    };

    // 2. Destroy proxy for this session - ALWAYS do this!
    // This prevents resource leaks (MCP processes) even if the session wasn't active
    // (e.g. failed resume or already closed in memory but proxy still exists).
    proxy_manager.destroy_proxy(&session_id).await;
    log::info!("Destroyed MCP proxy for session: {}", session_id);

    // 3. Update status to idle in DB (and memory if active)
    // We ignore error here for inactive sessions to ensure best-effort cleanup
    let _ = crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Idle,
    )
    .await;

    // 4. Cleanup memory state if active
    if session_active {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            session.is_running = false;
            // Reset cancellation token for potential future workflows
            session.cancellation_token = CancellationToken::new();
        }
    }

    // 5. Emit workflow stopped event
    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.clone(),
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    log::info!("Terminated workflow for session: {}", session_id);

    if !session_active {
        // Return error for non-active sessions to maintain backward compatibility with callers
        // that expect to know if the session was not in memory, but AFTER cleanup.
        return Err(format!("Session not found: {}", session_id));
    }

    Ok(())
}

/// Cancel a running workflow
/// This triggers the cancellation token to abort any running operations
pub async fn cancel_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    _proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    log::info!("Cancelling workflow for session: {}", session_id);

    // Determine whether to stop immediately or defer to message boundary.
    // If a tool-call batch is in progress, we only set cancel_pending and let
    // continue_workflow_after_tool consume it after the full message completes.
    let has_pending_execution = {
        let active = active_sessions.read().await;
        let session = active
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        session.cancel_pending.store(true, Ordering::SeqCst);
        session.pending_execution.is_some()
    };

    if classify_cancel_strategy(has_pending_execution) == CancelStrategy::DeferToMessageBoundary {
        log::info!(
            "Cancel requested for session {} (deferred to message boundary)",
            session_id
        );
        // Discard any user messages that arrived while the agent was busy and
        // are waiting in the pending_events queue. They must not be processed
        // after the agent stops, regardless of when the active tool batch
        // completes. The tool batch itself is still allowed to finish cleanly.
        discard_pending_events(active_sessions, &session_id).await;
        // SP6: Wake any awaitAgent/pollProcess waiter that is suspended inside
        // a tool call for THIS session. The deferred cancel only sets
        // cancel_pending; without this notification the waiter would sleep up
        // to 30 s before re-checking the flag.
        crate::state::get_session_bus().notify_status_change(&session_id);
        return Ok(());
    }

    // No in-flight tool-call batch: stop immediately.
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Idle,
    )
    .await?;

    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = false;
        session.cancel_pending.store(false, Ordering::SeqCst);
        // Cancel the token (do NOT replace with a fresh one yet).
        // The cancelled state persists until start_workflow explicitly resets it.
        // This prevents stale in-flight LLM responses carrying tool_calls from
        // re-entering the workflow via the Idle+allow_idle_tool_entry path after cancel.
        session.cancellation_token.cancel();
    }
    drop(active);

    discard_pending_events(active_sessions, &session_id).await;

    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.clone(),
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    log::info!("Cancelled workflow for session: {}", session_id);
    Ok(())
}

/// Helper to handle tool result and trigger next steps if valid
pub async fn continue_workflow_after_tool(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    tool_call_id: String,
    result: crate::commands::agent_commands::ToolExecutionResult,
) -> Result<(), String> {
    use crate::mcp::types::MCPContent;

    match crate::agent::tools::handle_tool_result(
        active_sessions,
        app_handle,
        session_id.clone(),
        tool_call_id,
        result,
    )
    .await
    {
        Ok(Some(accumulated_messages)) => {
            log::info!(
                "All tool results received for session {}. Proceeding.",
                session_id
            );

            // Use MessageService to handle message caching, event emission, and DB persistence.
            // Propagate errors so the LLM loop does not continue with a stale context window
            // if injection fails (e.g. due to a DB initialization error).
            crate::services::MessageService::inject_messages_to_session(
                active_sessions,
                app_handle,
                &session_id,
                accumulated_messages.clone(),
                true,
            )
            .await
            .map_err(|e| {
                log::error!(
                    "Failed to inject tool result messages into session cache: {}",
                    e
                );
                e
            })?;

            // Message-boundary cancel handling:
            // If cancel was requested while tools were running, consume it now
            // after this message's full tool-call batch has completed.
            let should_stop_after_message = {
                let sessions = active_sessions.read().await;
                sessions
                    .get(&session_id)
                    .map(|session| session.cancel_pending.load(Ordering::SeqCst))
                    .unwrap_or(false)
            };

            if should_consume_cancel_at_message_boundary(should_stop_after_message) {
                log::info!(
                    "Consumed pending cancel at message boundary for session {}",
                    session_id
                );

                {
                    let mut sessions = active_sessions.write().await;
                    if let Some(session) = sessions.get_mut(&session_id) {
                        session.cancel_pending.store(false, Ordering::SeqCst);
                        session.is_running = false;
                        session.cancellation_token = CancellationToken::new();
                    }
                }

                discard_pending_events(active_sessions, &session_id).await;

                let _ = crate::agent::lifecycle::update_session_status(
                    session_repo,
                    active_sessions,
                    app_handle,
                    &session_id,
                    SessionStatus::Idle,
                )
                .await;

                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.clone(),
                };
                let _ = crate::agent::events::emit_agent_event(app_handle, event);
                return Ok(());
            }

            // Check for UI interaction (stop condition)
            let has_ui_interaction = accumulated_messages.iter().any(|msg| {
                msg.content
                    .iter()
                    .any(|c| matches!(c, MCPContent::Resource { .. }))
            });

            if has_ui_interaction {
                log::info!(
                    "UI interaction detected for session {}. Stopping loop.",
                    session_id
                );
                let _ = crate::agent::lifecycle::update_session_status(
                    session_repo,
                    active_sessions,
                    app_handle,
                    &session_id,
                    SessionStatus::Idle,
                )
                .await;
                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.clone(),
                };
                let _ = crate::agent::events::emit_agent_event(app_handle, event);
            } else {
                // Check status before requesting LLM completion (Defense in depth against race condition)
                {
                    let sessions = active_sessions.read().await;
                    if let Some(session) = sessions.get(&session_id) {
                        if session.metadata.status != crate::repositories::SessionStatus::Busy {
                            log::info!(
                                "Skipping workflow restart for session {} (status: {:?})",
                                session_id,
                                session.metadata.status
                            );
                            return Ok(());
                        }
                    }
                }

                // Request next LLM completion
                if let Err(e) = crate::agent::llm::request_llm_completion(
                    session_repo,
                    active_sessions,
                    proxy_manager,
                    app_handle,
                    session_id,
                )
                .await
                {
                    log::error!("Failed to request LLM completion: {}", e);
                    return Err(format!("Failed to request LLM completion: {}", e));
                }
            }
        }
        Ok(Option::None) => {
            // Still waiting for other tools
        }
        Err(e) => {
            // Handle cancellation gracefully without emitting error event
            if e == "Workflow was cancelled" {
                log::info!(
                    "Ignoring tool result for session {} because the workflow was cancelled",
                    session_id
                );
                return Err(e);
            }

            log::error!("Error handling tool result: {}", e);
            if let Err(err) = crate::agent::llm::handle_llm_error(
                session_repo,
                active_sessions,
                app_handle,
                session_id,
                e.clone(),
            )
            .await
            {
                log::error!("Failed to handle LLM error: {}", err);
            }
            return Err(e);
        }
    }
    Ok(())
}

async fn discard_pending_events(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let mut messages_to_delete = Vec::new();

    // 1. Drain from pending events queue and remove from in-memory cache
    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let mut pending_events = session.pending_events.write().await;
            messages_to_delete = pending_events.drain_messages();

            if !messages_to_delete.is_empty() {
                let mut messages = session.messages.write().await;
                // Remove these messages from the cache
                messages.retain(|m| !messages_to_delete.contains(&m.id));

                log::info!(
                    "Cleared {} pending events from queue and cache for session {}",
                    messages_to_delete.len(),
                    session_id
                );
            }
        }
    }

    // 2. Delete from database
    if !messages_to_delete.is_empty() {
        let repo = crate::state::get_message_repository();
        for msg_id in messages_to_delete {
            if let Err(e) = repo.delete_by_id(&msg_id).await {
                log::error!(
                    "Failed to delete cancelled pending message {} from DB: {}",
                    msg_id,
                    e
                );
            }
        }
    }
}
