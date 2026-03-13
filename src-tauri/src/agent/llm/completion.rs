use crate::agent::references::build_default_registry;
use crate::agent::state::AgentSession;
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use super::prompt::build_session_system_prompt_split;
use super::types::{CompactRequest, CompletionRequest};

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
    let session_name = session
        .metadata
        .name
        .clone()
        .unwrap_or_else(|| session_id[..8.min(session_id.len())].to_string());

    let temperature = agent_config.temperature;
    let max_tokens = agent_config.max_tokens;

    drop(active);

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
    // --- CONTEXT MANAGEMENT ---
    let messages = merge_consecutive_user_messages(messages);

    let settings_repo = crate::state::get_settings_repository();
    let settings_val = settings_repo.get("settings").await.unwrap_or(None);

    let mut context_strategy = "compact".to_string();
    let mut window_size = 20;
    let mut max_input_context = 49152;
    let mut tool_call_group_visible_count = 4;
    let model_max_limit = 128_000; // Simplified default fallback

    if let Some(model) = settings_val {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&model.value) {
            if let Some(strategy) = json.get("contextStrategy").and_then(|v| v.as_str()) {
                context_strategy = strategy.to_string();
            }
            if let Some(ws) = json.get("windowSize").and_then(|v| v.as_u64()) {
                window_size = ws as usize;
            }
            if let Some(mic) = json.get("maxInputContext").and_then(|v| v.as_u64()) {
                max_input_context = mic as usize;
            }
            if let Some(tcgvc) = json
                .get("toolCallGroupVisibleCount")
                .and_then(|v| v.as_u64())
            {
                tool_call_group_visible_count = tcgvc as usize;
            }
        }
    }

    let safe_input_token_limit = std::cmp::min(max_input_context, model_max_limit);

    let tools_json = available_tools
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_default());
    let system_prompt_tokens = system_prompt
        .as_ref()
        .map(|s| crate::agent::llm::token_utils::estimate_text_tokens(s))
        .unwrap_or(0);
    let session_context_tokens = session_context
        .as_ref()
        .map(|s| crate::agent::llm::token_utils::estimate_text_tokens(s))
        .unwrap_or(0);
    let tools_tokens = tools_json
        .as_ref()
        .map(|s| crate::agent::llm::token_utils::estimate_text_tokens(s))
        .unwrap_or(0);

    // --- Step A: Inject compact summary (if a valid record is cached) ---
    // Clone Arc refs while holding the outer read lock, then release it immediately.
    let (compact_context_arc, compact_in_flight_arc) = {
        let active = active_sessions.read().await;
        let session = active
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.compact_context.clone(),
            session.compact_in_flight.clone(),
        )
    };

    let messages = {
        let compact_record = compact_context_arc.read().await.clone();
        if let Some(record) = compact_record {
            if let Some(to_idx) = messages.iter().position(|m| m.id == record.to_id) {
                let now_ms = chrono::Utc::now().timestamp_millis();
                let summary_msg = Message {
                    id: format!("compact-summary-{}", session_id),
                    session_id: session_id.clone(),
                    role: "user".to_string(),
                    content: vec![MCPContent::Text {
                        text: format!("### Previous Conversation Summary\n\n{}", record.summary),
                        is_error: None,
                    }],
                    source: Some("compact-summary".to_string()),
                    created_at: now_ms,
                    updated_at: now_ms,
                    tool_calls: None,
                    tool_call_id: None,
                    is_streaming: None,
                    thinking: None,
                    thinking_signature: None,
                    assistant_id: None,
                    attachments: None,
                    tool_use: None,
                    usage: None,
                    error: None,
                    metadata: None,
                };
                let tail = messages[(to_idx + 1)..].to_vec();
                log::info!(
                    "📦 Injected compact summary: session={}, toId={}, tail_count={}",
                    session_id,
                    record.to_id,
                    tail.len()
                );
                [vec![summary_msg], tail].concat()
            } else {
                // Stale: to_id not found in current message stack — invalidate in-memory cache.
                *compact_context_arc.write().await = None;
                log::warn!(
                    "⚠️ Compact cache stale (toId not found), invalidated: session={}",
                    session_id
                );
                messages
            }
        } else {
            messages
        }
    };

    let mut final_messages = messages.clone();

    let combined_system_prompt = match (&system_prompt, &session_context) {
        (Some(sp), Some(sc)) => Some(format!("{}\n\n{}", sp, sc)),
        (Some(sp), None) => Some(sp.clone()),
        (None, Some(sc)) => Some(sc.clone()),
        (None, None) => None,
    };

    if context_strategy == "compact" {
        let threshold =
            crate::agent::llm::token_utils::calculate_compact_threshold(safe_input_token_limit);
        let current_tokens = crate::agent::llm::token_utils::calculate_grounded_total_tokens(
            &messages,
            system_prompt_tokens + session_context_tokens,
            tools_tokens,
        );

        if current_tokens > threshold {
            let split_idx = crate::agent::llm::context_selector::find_compaction_split_index(
                &messages,
                threshold,
                system_prompt_tokens + session_context_tokens,
                tools_tokens,
            );
            if split_idx > 0 && split_idx < messages.len() {
                // --- Step B: Guards G1 + G2, then fire-and-forget compact trigger ---
                let in_flight = compact_in_flight_arc.load(Ordering::SeqCst);
                let cached_to_id = compact_context_arc
                    .read()
                    .await
                    .as_ref()
                    .map(|r| r.to_id.clone());
                let pending_to_id = messages.get(split_idx - 1).map(|m| m.id.clone());

                if !in_flight && cached_to_id.as_deref() != pending_to_id.as_deref() {
                    let compact_msgs = messages[..split_idx].to_vec();
                    let from_id = compact_msgs
                        .first()
                        .map(|m| m.id.clone())
                        .unwrap_or_default();
                    let to_id = compact_msgs
                        .last()
                        .map(|m| m.id.clone())
                        .unwrap_or_default();

                    // Set flag synchronously before spawning to prevent TOCTOU.
                    compact_in_flight_arc.store(true, Ordering::SeqCst);

                    let compact_event = CompactRequest {
                        session_id: session_id.clone(),
                        session_name: session_name.clone(),
                        messages: compact_msgs,
                        from_id,
                        to_id,
                    };
                    let app = app_handle.clone();
                    tokio::spawn(async move {
                        if let Err(e) = app.emit("llm:compact-request", compact_event) {
                            log::error!("Failed to emit llm:compact-request: {}", e);
                        }
                    });
                    log::info!(
                        "🔧 Compaction triggered: session={}, split_idx={}",
                        session_id,
                        split_idx
                    );
                } else {
                    log::debug!(
                        "⏭️ Compaction skipped: session={}, in_flight={}, same_range={}",
                        session_id,
                        in_flight,
                        cached_to_id.as_deref() == pending_to_id.as_deref()
                    );
                }
            }
        }

        let context_options = crate::agent::llm::context_selector::SelectionOptions {
            system_prompt: combined_system_prompt.clone(),
            tools_json: tools_json.clone(),
            max_messages: None,
            max_tool_calls_per_message: Some(if provider == "gemini" {
                100
            } else {
                tool_call_group_visible_count
            }),
        };

        final_messages = crate::agent::llm::context_selector::select_messages_within_context(
            &final_messages,
            &provider,
            Some(safe_input_token_limit),
            Some(&context_options),
            Some(&crate::agent::llm::context_selector::ModelContextInfo {
                context_window: model_max_limit,
            }),
        );
    } else {
        let context_options = crate::agent::llm::context_selector::SelectionOptions {
            system_prompt: combined_system_prompt.clone(),
            tools_json: tools_json.clone(),
            max_messages: Some(window_size),
            max_tool_calls_per_message: Some(if provider == "gemini" {
                100
            } else {
                tool_call_group_visible_count
            }),
        };

        final_messages = crate::agent::llm::context_selector::select_messages_within_context(
            &messages,
            &provider,
            Some(safe_input_token_limit),
            Some(&context_options),
            Some(&crate::agent::llm::context_selector::ModelContextInfo {
                context_window: model_max_limit,
            }),
        );
    }

    let total_estimated_tokens = crate::agent::llm::token_utils::calculate_grounded_total_tokens(
        &final_messages,
        system_prompt_tokens + session_context_tokens,
        tools_tokens,
    );

    let context_usage = Some(serde_json::json!({
        "totalTokens": total_estimated_tokens,
        "contextWindow": safe_input_token_limit,
        "modelMaxContext": model_max_limit,
    }));

    let request = CompletionRequest {
        session_id: session_id.clone(),
        messages: final_messages,
        model,
        provider,
        system_prompt,
        session_context,
        temperature,
        max_tokens,
        available_tools,
        context_usage,
    };

    app_handle
        .emit("llm:completion-request", request)
        .map_err(|e| format!("Failed to emit LLM completion request: {}", e))?;

    log::info!("Emitted LLM completion request for session: {}", session_id);

    Ok(())
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
