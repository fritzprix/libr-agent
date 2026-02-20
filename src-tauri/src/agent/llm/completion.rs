use crate::agent::state::AgentSession;
use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use super::types::CompletionRequest;
use super::prompt::build_session_system_prompt;

/// Request LLM completion from frontend
///
/// Note: session_repo is passed through to handle_llm_response which uses it for status updates
pub async fn request_llm_completion(
    _session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    // 1. Validate session status before proceeding (Race Condition Fix)
    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            if session.cancel_pending.load(Ordering::SeqCst)
                || session.metadata.status != SessionStatus::Busy
            {
                log::info!(
                    "Rejecting LLM request for session {} (status: {:?}, cancel_pending={})",
                    session_id,
                    session.metadata.status,
                    session.cancel_pending.load(Ordering::SeqCst)
                );
                return Err(format!(
                    "Cannot request LLM completion: session status is {:?}",
                    session.metadata.status
                ));
            }
        } else {
            return Err(format!("Session not found: {}", session_id));
        }
    }

    // Emit MessageAdded events for any pending user messages before LLM request
    // This makes them visible in the frontend (removed from pendingMessages queue)
    // Optimization: Collect all data first, then release locks before I/O
    let pending_messages: Vec<Message> = {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            let mut pending_events = session.pending_events.write().await;
            let pending_ids = pending_events.drain_messages();

            if pending_ids.is_empty() {
                Vec::new()
            } else {
                let messages = session.messages.read().await;

                // Build HashMap for O(1) lookup instead of O(n) iter().find()
                let msg_map: std::collections::HashMap<&str, &Message> =
                    messages.iter().map(|m| (m.id.as_str(), m)).collect();

                // Collect messages matching pending IDs
                pending_ids
                    .iter()
                    .filter_map(|id| msg_map.get(id.as_str()).map(|&m| m.clone()))
                    .collect()
            }
        } else {
            Vec::new()
        }
    }; // All locks released here

    // Now emit events without holding any locks
    for msg in pending_messages {
        let event = crate::agent::events::AgentEvent::MessageAdded {
            session_id: session_id.clone(),
            message: Box::new(msg.clone()),
        };
        let _ = crate::agent::events::emit_agent_event(app_handle, event);
        log::info!(
            "Emitted MessageAdded for previously pending message: {}",
            msg.id
        );
    }

    // Read messages from in-memory cache, excluding recovery tombstones (source="recovery")
    let messages = {
        let sessions = active_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let messages_lock = session.messages.read().await;
        messages_lock
            .iter()
            .filter(|m| m.source.as_deref() != Some("recovery"))
            .cloned()
            .collect::<Vec<_>>()
    };

    log::info!(
        "🔄 Message stack for LLM request: session={}, count={}, first_msg_id={}, last_msg_id={}",
        session_id,
        messages.len(),
        messages.first().map(|m| m.id.as_str()).unwrap_or("none"),
        messages.last().map(|m| m.id.as_str()).unwrap_or("none")
    );

    // Get agent config
    let active = active_sessions.read().await;
    let session = active
        .get(&session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let agent_config = session
        .metadata
        .agent_config
        .as_ref()
        .ok_or_else(|| "Agent configuration is required but not found".to_string())
        .and_then(|json| crate::agent::AgentConfig::from_json(json).map_err(|e| e.to_string()))?;

    let model = session.metadata.model.clone();
    let provider = session.metadata.provider.clone();

    let temperature = Some(agent_config.temperature);
    let max_tokens = agent_config.max_tokens;

    drop(active);

    // Build system prompt
    let system_prompt =
        Some(build_session_system_prompt(active_sessions, proxy_manager, &session_id).await?);

    // Collect available tools
    let available_tools =
        crate::agent::tools::collect_available_tools(&session_id, &agent_config, proxy_manager)
            .await
            .ok();

    let request = CompletionRequest {
        session_id: session_id.clone(),
        messages,
        model,
        provider,
        system_prompt,
        temperature,
        max_tokens,
        available_tools,
    };

    app_handle
        .emit("llm:completion-request", request)
        .map_err(|e| format!("Failed to emit LLM completion request: {}", e))?;

    log::info!("Emitted LLM completion request for session: {}", session_id);

    Ok(())
}
