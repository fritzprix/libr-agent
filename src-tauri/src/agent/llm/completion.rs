use crate::agent::references::build_default_registry;
use crate::agent::state::AgentSession;
use crate::agent::{compact, compact::CompletionPreparation};
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use super::prompt::build_session_system_prompt_split;
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

    // 2. Drain and integrate pending user messages BEFORE preparing the message stack
    // This ensures they are appended AFTER the last assistant/tool response
    let pending_messages: Vec<Message> = {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            let mut pending_events = session.pending_events.write().await;
            let pending_ids = pending_events.drain_messages();

            if pending_ids.is_empty() {
                Vec::new()
            } else {
                // Fetch from DB since they are not in session.messages yet
                let repo = crate::state::get_message_repository();
                match repo.get_by_ids(pending_ids.clone()).await {
                    Ok(msgs) => {
                        log::info!(
                            "Drained {} pending messages from queue for session {}",
                            msgs.len(),
                            session_id
                        );

                        // Push to session cache so they are included in the 'messages' read below
                        let mut messages_lock = session.messages.write().await;
                        for msg in &msgs {
                            messages_lock.push(msg.clone());
                            if messages_lock.len() > crate::agent::state::MAX_CACHED_MESSAGES {
                                messages_lock.remove(0);
                            }
                        }
                        msgs
                    }
                    Err(e) => {
                        log::error!("Failed to fetch pending messages from DB: {}", e);
                        Vec::new()
                    }
                }
            }
        } else {
            Vec::new()
        }
    }; // All locks released here

    // Emit MessageAdded events for the now-integrated messages
    for msg in pending_messages {
        let event = crate::agent::events::AgentEvent::MessageAdded {
            session_id: session_id.clone(),
            message: Box::new(msg.clone()),
        };
        let _ = crate::agent::events::emit_agent_event(app_handle, event);
    }

    // 3. Read messages from in-memory cache (now includes the drained pending messages)
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

    // Get agent config and current compact state without holding the session map lock
    // across awaits on the per-session compact_context lock.
    let (metadata, compact_context_lock) = {
        let active = active_sessions.read().await;
        let session = active
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.metadata.clone(),
            Arc::clone(&session.compact_context),
        )
    };

    let agent_config = metadata
        .agent_config
        .as_ref()
        .ok_or_else(|| "Agent configuration is required but not found".to_string())
        .and_then(|json| crate::agent::AgentConfig::from_json(json).map_err(|e| e.to_string()))?;

    let model = metadata.model.clone();
    let provider = metadata.provider.clone();
    let compact_record = compact_context_lock.read().await.clone();

    let temperature = agent_config.temperature;
    let max_tokens = agent_config.max_tokens;

    // Build system prompt split: stable prefix (cacheable) + volatile session context (per-turn).
    // The frontend AI service layer receives both parts and decides the injection strategy via
    // `prepareContextInjection` — providers may concatenate (default) or inject context as an
    // ephemeral message for better prefix-cache utilization (e.g. OpenAI).
    let (stable_prompt, session_context) =
        build_session_system_prompt_split(active_sessions, proxy_manager, &session_id).await?;
    let system_prompt = Some(stable_prompt);

    // Collect available tools
    let available_tools =
        crate::agent::tools::collect_available_tools(&session_id, &agent_config, proxy_manager)
            .await
            .ok();

    // Resolve @type:arg references in user messages (Late Binding).
    // The stored messages are NOT modified — only the CompletionRequest payload is enriched.
    let messages =
        resolve_message_references(messages, &session_id, agent_config.id.as_deref()).await;

    // Merge any consecutive user messages that may result from crash recovery
    // (unanswered user turn followed by a new user message). This is the only
    // place where consecutive user roles are legitimate; merging here keeps the
    // Gemini mapper simple and mapper-agnostic.
    let messages = merge_consecutive_user_messages(messages);

    let preparation = compact::prepare_completion_request(compact::PrepareCompletionInput {
        session_id: session_id.clone(),
        messages,
        model,
        provider,
        system_prompt,
        session_context,
        temperature,
        max_tokens,
        available_tools,
        compact_record,
    })
    .await?;

    match preparation {
        CompletionPreparation::Ready(prepared) => {
            if prepared.invalidate_compact_record {
                invalidate_stale_compact_record(active_sessions, &session_id).await?;
            }

            if let Some(state) = &prepared.state {
                compact::emit_compaction_state(app_handle, state)?;
            }

            app_handle
                .emit("llm:completion-request", prepared.request)
                .map_err(|e| format!("Failed to emit LLM completion request: {}", e))?;

            log::info!("Emitted LLM completion request for session: {}", session_id);
        }
        CompletionPreparation::NeedsCompaction(pending) => {
            if pending.invalidate_compact_record {
                invalidate_stale_compact_record(active_sessions, &session_id).await?;
            }

            let pending_compaction_lock = {
                let active = active_sessions.read().await;
                let session = active
                    .get(&session_id)
                    .ok_or_else(|| format!("Session not found: {}", session_id))?;
                Arc::clone(&session.pending_compaction)
            };

            let mut slot = pending_compaction_lock.write().await;

            // Emit first; only persist pending state after both succeed so a
            // failed emit doesn't leave a stale slot with nothing to resolve it.
            compact::emit_compaction_state(app_handle, &pending.state)?;
            app_handle
                .emit("llm:compaction-request", pending.request)
                .map_err(|e| format!("Failed to emit compaction request: {}", e))?;

            *slot = Some(pending.pending);

            log::info!("Emitted LLM compaction request for session: {}", session_id);
        }
    }

    Ok(())
}

async fn invalidate_stale_compact_record(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(), String> {
    let compact_context_lock = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| Arc::clone(&session.compact_context))
    };

    if let Some(compact_context) = compact_context_lock {
        let mut compact = compact_context.write().await;
        *compact = None;
    }

    let repo = crate::state::get_compact_context_repository();
    repo.delete_by_session_id(session_id)
        .await
        .map_err(|error| error.to_string())
}

/// Resolve `@type:arg` references in user messages.
/// Each user message's text content is processed through the reference registry.
/// Only the returned `Vec<Message>` is modified — the session store is untouched.
async fn resolve_message_references(
    messages: Vec<Message>,
    session_id: &str,
    assistant_id: Option<&str>,
) -> Vec<Message> {
    let registry = build_default_registry(session_id, assistant_id).await;
    let mut result = Vec::with_capacity(messages.len());

    for mut msg in messages {
        if msg.role == "user" {
            let mut new_content: Vec<MCPContent> = Vec::with_capacity(msg.content.len());
            for part in msg.content {
                if let MCPContent::Text { text, .. } = &part {
                    // Only process parts that contain @ references
                    if text.contains('@') {
                        let resolved = registry.preprocess_message_text(text).await;
                        new_content.push(MCPContent::Text {
                            text: resolved,
                            is_error: None,
                        });
                    } else {
                        new_content.push(part);
                    }
                } else {
                    new_content.push(part);
                }
            }
            msg.content = new_content;
        }
        result.push(msg);
    }
    result
}

/// Merge consecutive `user` role messages into a single message.
///
/// This is only expected after a crash-recovery scenario where an unanswered
/// user message sits at the tail of history and the user sends another message
/// before the agent can respond. The content of subsequent user messages is
/// appended to the first with a separator. IDs and metadata from the first
/// message are preserved. This operates on the CompletionRequest payload only —
/// stored messages are never mutated.
fn merge_consecutive_user_messages(messages: Vec<Message>) -> Vec<Message> {
    let mut result: Vec<Message> = Vec::with_capacity(messages.len());

    for msg in messages {
        if msg.role == "user" {
            if let Some(last) = result.last_mut() {
                if last.role == "user" {
                    // Append a separator followed by the new content
                    last.content.push(MCPContent::Text {
                        text: "\n\n---\n\n".to_string(),
                        is_error: None,
                    });
                    last.content.extend(msg.content);
                    log::info!(
                        "Merged consecutive user messages: base={}, appended={}",
                        last.id,
                        msg.id
                    );
                    continue;
                }
            }
        }
        result.push(msg);
    }

    result
}
