use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{MessageRepository, SessionStatus};
use std::collections::HashMap;
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

    // 3. Persist to DB synchronously to ensure data integrity
    // We await this to prevent "ghost messages" where memory has state but DB doesn't (causing data loss on reload)
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
    // If the session was previously terminated, the proxy might have been destroyed.
    // We must recreate it to ensure the LLM gets the full context (tools, etc).
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
        session_repo,
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
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Idle,
    )
    .await?;

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

            // Add to cache
            {
                let sessions = active_sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    let mut messages = session.messages.write().await;
                    for msg in &accumulated_messages {
                        messages.push(msg.clone());
                        if messages.len() > MAX_CACHED_MESSAGES {
                            messages.remove(0);
                        }
                    }
                }
            }

            // Emit MessageAdded for each
            for msg in &accumulated_messages {
                let event = crate::agent::events::AgentEvent::MessageAdded {
                    session_id: session_id.clone(),
                    message: Box::new(msg.clone()),
                };
                let _ = crate::agent::events::emit_agent_event(app_handle, event);
            }

            // Persist to DB
            let msgs_for_db = accumulated_messages.clone();

            tokio::spawn(async move {
                let repo = crate::state::get_message_repository();
                for msg in msgs_for_db {
                    if let Err(e) = repo.insert(&msg).await {
                        log::error!("Failed to persist tool result message: {}", e);
                    }
                }
            });

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
