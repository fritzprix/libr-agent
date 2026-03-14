use crate::agent::references::build_default_registry;
use crate::agent::state::AgentSession;
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::CompactContextRepository;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use super::prompt::build_session_system_prompt_split;
use super::types::{AgentRuntimeError, AgentRuntimeErrorType, CompactRequest, CompletionRequest};

#[derive(Debug)]
struct OverflowPreflight {
    latest_message_tokens: usize,
    total_tokens: usize,
    reserved_tokens: usize,
    safety_margin: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextManagementSettings {
    context_strategy: String,
    window_size: usize,
    max_input_context: usize,
    tool_call_group_visible_count: usize,
    model_max_limit: usize,
}

impl ContextManagementSettings {
    pub fn context_strategy(&self) -> &str {
        &self.context_strategy
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }

    pub fn max_input_context(&self) -> usize {
        self.max_input_context
    }

    pub fn tool_call_group_visible_count(&self) -> usize {
        self.tool_call_group_visible_count
    }
}

fn default_context_management_settings() -> ContextManagementSettings {
    ContextManagementSettings {
        context_strategy: "compact".to_string(),
        window_size: 20,
        max_input_context: 49152,
        tool_call_group_visible_count: 4,
        model_max_limit: 128_000,
    }
}

fn apply_context_management_setting(
    settings: &mut ContextManagementSettings,
    key: &str,
    value: &serde_json::Value,
) {
    match key {
        "contextStrategy" => {
            if let Some(strategy) = value.as_str() {
                settings.context_strategy = strategy.to_string();
            }
        }
        "windowSize" => {
            if let Some(window_size) = value.as_u64() {
                settings.window_size = window_size as usize;
            }
        }
        "maxInputContext" => {
            if let Some(max_input_context) = value.as_u64() {
                settings.max_input_context = max_input_context as usize;
            }
        }
        "toolCallGroupVisibleCount" => {
            if let Some(visible_count) = value.as_u64() {
                settings.tool_call_group_visible_count = visible_count as usize;
            }
        }
        _ => {}
    }
}

pub fn resolve_context_management_settings(
    legacy_settings_blob: Option<&serde_json::Value>,
    direct_settings: &HashMap<String, serde_json::Value>,
) -> ContextManagementSettings {
    let mut settings = default_context_management_settings();

    if let Some(legacy_blob) = legacy_settings_blob {
        if let Some(legacy_object) = legacy_blob.as_object() {
            for (key, value) in legacy_object {
                apply_context_management_setting(&mut settings, key, value);
            }
        }
    }

    for (key, value) in direct_settings {
        apply_context_management_setting(&mut settings, key, value);
    }

    settings
}

pub(crate) async fn load_context_management_settings() -> ContextManagementSettings {
    let settings_repo = crate::state::get_settings_repository();
    let legacy_settings_blob = settings_repo
        .get("settings")
        .await
        .unwrap_or(None)
        .and_then(|model| serde_json::from_str::<serde_json::Value>(&model.value).ok());

    let mut direct_settings = HashMap::new();
    for key in [
        "contextStrategy",
        "windowSize",
        "maxInputContext",
        "toolCallGroupVisibleCount",
    ] {
        if let Some(model) = settings_repo.get(key).await.unwrap_or(None) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&model.value) {
                direct_settings.insert(key.to_string(), value);
            }
        }
    }

    resolve_context_management_settings(legacy_settings_blob.as_ref(), &direct_settings)
}

pub fn uses_compaction_strategy(context_strategy: &str) -> bool {
    context_strategy == "compact"
}

pub fn should_trigger_background_compaction(
    current_tokens: usize,
    safe_input_token_limit: usize,
    context_strategy: &str,
) -> bool {
    uses_compaction_strategy(context_strategy)
        && current_tokens
            > crate::agent::llm::token_utils::calculate_compact_threshold(safe_input_token_limit)
}

async fn trigger_background_compaction(
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    compact_in_flight_arc: &Arc<AtomicBool>,
    last_compacted_tail_id_arc: &Arc<RwLock<Option<String>>>,
) -> Result<bool, String> {
    let split_idx = crate::agent::llm::context_selector::find_compaction_split_index(messages);
    if split_idx == 0 {
        return Ok(false);
    }

    let claimed_in_flight = compact_in_flight_arc
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();

    if !claimed_in_flight {
        log::debug!("⏭️ Compaction skipped (in_flight): session={}", session_id);
        return Ok(false);
    }

    let current_tail_id = messages.last().map(|m| m.id.clone());
    let last_compacted_tail = last_compacted_tail_id_arc.read().await.clone();
    let same_tail = current_tail_id.as_deref() == last_compacted_tail.as_deref();

    if same_tail {
        compact_in_flight_arc.store(false, Ordering::SeqCst);
        log::debug!(
            "⏭️ Compaction skipped (same tail): session={}, tail={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?")
        );
        return Ok(false);
    }

    let compact_msgs = messages[..split_idx].to_vec();
    let from_id = compact_msgs
        .first()
        .map(|m| m.id.clone())
        .unwrap_or_default();
    let to_id = compact_msgs
        .last()
        .map(|m| m.id.clone())
        .unwrap_or_default();

    *last_compacted_tail_id_arc.write().await = current_tail_id.clone();

    let compact_event = CompactRequest {
        session_id: session_id.to_string(),
        session_name: session_name.to_string(),
        messages: compact_msgs,
        from_id,
        to_id,
        resume_completion_after_compact: false,
    };
    let app = app_handle.clone();
    let state_session_id = session_id.to_string();
    let state_session_name = session_name.to_string();
    tokio::spawn(async move {
        let state_event = crate::agent::llm::types::CompactStateEvent {
            session_id: state_session_id.clone(),
            session_name: Some(state_session_name),
            compacting: true,
            phase: crate::agent::llm::types::CompactStatePhase::Started,
        };
        if let Err(e) = app.emit("llm:compact-state", state_event) {
            log::error!("Failed to emit llm:compact-state: {}", e);
        }
        if let Err(e) = app.emit("llm:compact-request", compact_event) {
            log::error!("Failed to emit llm:compact-request: {}", e);
        }
    });
    log::info!(
        "🔧 Compaction triggered: session={}, split_idx={}, tail={}",
        session_id,
        split_idx,
        current_tail_id.as_deref().unwrap_or("?")
    );

    Ok(true)
}

pub async fn maybe_trigger_post_idle_compaction(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    usage_total_tokens: usize,
) -> Result<bool, String> {
    let settings = load_context_management_settings().await;
    let safe_input_token_limit =
        std::cmp::min(settings.max_input_context, settings.model_max_limit);

    if !should_trigger_background_compaction(
        usage_total_tokens,
        safe_input_token_limit,
        &settings.context_strategy,
    ) {
        return Ok(false);
    }

    let (compact_context_arc, compact_in_flight_arc, last_compacted_tail_id_arc) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.compact_context.clone(),
            session.compact_in_flight.clone(),
            session.last_compacted_tail_id.clone(),
        )
    };

    if compact_context_arc.read().await.is_some() {
        log::debug!(
            "⏭️ Post-idle compaction skipped (summary cached): session={}",
            session_id
        );
        return Ok(false);
    }

    let triggered = trigger_background_compaction(
        app_handle,
        session_id,
        session_name,
        messages,
        &compact_in_flight_arc,
        &last_compacted_tail_id_arc,
    )
    .await?;

    if triggered {
        log::info!(
            "🧹 Post-idle compaction triggered from completed response usage: session={}, total_tokens={}, limit={}",
            session_id,
            usage_total_tokens,
            safe_input_token_limit
        );
    }

    Ok(triggered)
}

/// Request LLM completion from frontend
///
/// Note: session_repo is passed through to handle_llm_response which uses it for status updates
pub async fn request_llm_completion(
    _session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), AgentRuntimeError> {
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
                return Err(AgentRuntimeError::new(
                    AgentRuntimeErrorType::AiServiceError,
                    format!(
                        "Cannot request LLM completion: session status is {:?}",
                        session.metadata.status
                    ),
                )
                .with_code("INVALID_SESSION_STATUS"));
            }
        } else {
            return Err(AgentRuntimeError::new(
                AgentRuntimeErrorType::AiServiceError,
                format!("Session not found: {}", session_id),
            )
            .with_code("SESSION_NOT_FOUND"));
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
        let session = sessions.get(&session_id).ok_or_else(|| {
            AgentRuntimeError::new(
                AgentRuntimeErrorType::AiServiceError,
                format!("Session not found: {}", session_id),
            )
            .with_code("SESSION_NOT_FOUND")
        })?;

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
    let session = active.get(&session_id).ok_or_else(|| {
        AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            format!("Session not found: {}", session_id),
        )
        .with_code("SESSION_NOT_FOUND")
    })?;

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

    let context_settings = load_context_management_settings().await;
    let context_strategy = context_settings.context_strategy.clone();
    let window_size = context_settings.window_size;
    let max_input_context = context_settings.max_input_context;
    let tool_call_group_visible_count = context_settings.tool_call_group_visible_count;
    let model_max_limit = context_settings.model_max_limit;

    // --- Step A: Inject compact summary (if a valid record is cached) ---
    // Clone Arc refs while holding the outer read lock, then release it immediately.
    let (compact_context_arc, compact_in_flight_arc, last_compacted_tail_id_arc) = {
        let active = active_sessions.read().await;
        let session = active.get(&session_id).ok_or_else(|| {
            AgentRuntimeError::new(
                AgentRuntimeErrorType::AiServiceError,
                format!("Session not found: {}", session_id),
            )
            .with_code("SESSION_NOT_FOUND")
        })?;
        (
            session.compact_context.clone(),
            session.compact_in_flight.clone(),
            session.last_compacted_tail_id.clone(),
        )
    };

    let messages = {
        let compact_record = compact_context_arc.read().await.clone();
        if let Some(record) = compact_record {
            if !uses_compaction_strategy(&context_strategy) {
                *compact_context_arc.write().await = None;
                let compact_repo = crate::state::get_compact_context_repository();
                if let Err(e) = compact_repo.delete_by_session_id(&session_id).await {
                    log::warn!(
                        "⚠️ Failed to delete compact cache while strategy={} for session {}: {}",
                        context_strategy,
                        session_id,
                        e
                    );
                }
                log::info!(
                    "🪟 Ignored and cleared stale compact cache because strategy={} for session {}",
                    context_strategy,
                    session_id
                );
                messages
            } else if let Some(to_idx) = messages.iter().position(|m| m.id == record.to_id) {
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
                let summary_tokens =
                    crate::agent::llm::token_utils::estimate_tokens_bpe(&summary_msg);
                let tail = messages[(to_idx + 1)..].to_vec();
                let tail_tokens: usize = tail
                    .iter()
                    .map(crate::agent::llm::token_utils::estimate_tokens_bpe)
                    .sum();
                log::info!(
                    "📦 Injected compact summary: session={}, toId={}, tail_count={}, summary_tokens={}, tail_tokens={}",
                    session_id,
                    record.to_id,
                    tail.len(),
                    summary_tokens,
                    tail_tokens
                );
                [vec![summary_msg], tail].concat()
            } else {
                // Stale: to_id not found in current message stack — invalidate in-memory cache
                // and delete the persisted record so future resume/cache hydration does not
                // keep reloading the same dead compact context forever.
                *compact_context_arc.write().await = None;
                let compact_repo = crate::state::get_compact_context_repository();
                if let Err(e) = compact_repo.delete_by_session_id(&session_id).await {
                    log::warn!(
                        "⚠️ Failed to delete stale compact cache for session {}: {}",
                        session_id,
                        e
                    );
                }
                log::warn!(
                    "⚠️ Compact cache stale (toId not found), invalidated + deleted: session={}",
                    session_id
                );
                messages
            }
        } else {
            messages
        }
    };

    let mut final_messages = messages.clone();
    let mut context_usage = None;

    let combined_system_prompt = match (&system_prompt, &session_context) {
        (Some(sp), Some(sc)) => Some(format!("{}\n\n{}", sp, sc)),
        (Some(sp), None) => Some(sp.clone()),
        (None, Some(sc)) => Some(sc.clone()),
        (None, None) => None,
    };

    if uses_compaction_strategy(&context_strategy) {
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

        let preflight = build_overflow_preflight(
            &messages,
            system_prompt_tokens + session_context_tokens,
            tools_tokens,
            safe_input_token_limit,
        );

        let latest_input_projected_tokens =
            preflight.reserved_tokens + preflight.latest_message_tokens + preflight.safety_margin;
        if latest_input_projected_tokens > safe_input_token_limit {
            let mut context = serde_json::Map::new();
            context.insert(
                "projectedTokens".to_string(),
                serde_json::json!(latest_input_projected_tokens),
            );
            context.insert(
                "effectiveLimit".to_string(),
                serde_json::json!(safe_input_token_limit),
            );
            return Err(
                AgentRuntimeError::new(
                    AgentRuntimeErrorType::ContextLimitError,
                    format!(
                        "Latest input is too large for the configured context window (projected {} > limit {}). Reduce the newest message or attachment payload and retry.",
                        latest_input_projected_tokens, safe_input_token_limit
                    ),
                )
                .with_code("LATEST_INPUT_TOO_LARGE")
                .with_context(context),
            );
        }

        let projected_total_tokens = preflight.total_tokens + preflight.safety_margin;
        if projected_total_tokens > safe_input_token_limit {
            if try_trigger_preflight_compaction(
                active_sessions,
                app_handle,
                &session_id,
                &session_name,
                &messages,
            )
            .await?
            {
                log::info!(
                    "⏸️ Pausing LLM request until compaction completes: session={}, projected_total={}, limit={}, margin={}",
                    session_id,
                    projected_total_tokens,
                    safe_input_token_limit,
                    preflight.safety_margin
                );
                return Ok(());
            }

            let mut context = serde_json::Map::new();
            context.insert(
                "projectedTokens".to_string(),
                serde_json::json!(projected_total_tokens),
            );
            context.insert(
                "effectiveLimit".to_string(),
                serde_json::json!(safe_input_token_limit),
            );
            return Err(
                AgentRuntimeError::new(
                    AgentRuntimeErrorType::ContextLimitError,
                    format!(
                        "Conversation context still exceeds the configured limit even after reserving safety margin (projected {} > limit {}). Wait for compaction or reduce recent input size.",
                        projected_total_tokens, safe_input_token_limit
                    ),
                )
                .with_code("CONTEXT_LIMIT_EXCEEDED")
                .with_context(context),
            );
        }

        let current_tokens = crate::agent::llm::token_utils::calculate_grounded_total_tokens(
            &messages,
            system_prompt_tokens + session_context_tokens,
            tools_tokens,
        );

        if should_trigger_background_compaction(
            current_tokens,
            safe_input_token_limit,
            &context_strategy,
        ) {
            let _ = trigger_background_compaction(
                app_handle,
                &session_id,
                &session_name,
                &messages,
                &compact_in_flight_arc,
                &last_compacted_tail_id_arc,
            )
            .await?;
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

        let total_estimated_tokens =
            crate::agent::llm::token_utils::calculate_grounded_total_tokens(
                &final_messages,
                system_prompt_tokens + session_context_tokens,
                tools_tokens,
            );

        context_usage = Some(serde_json::json!({
            "totalTokens": total_estimated_tokens,
            "contextWindow": safe_input_token_limit,
            "modelMaxContext": model_max_limit,
        }));
    } else {
        final_messages = crate::agent::llm::context_selector::select_recent_messages_fifo(
            &messages,
            &provider,
            window_size,
            if provider == "gemini" {
                100
            } else {
                tool_call_group_visible_count
            },
        );
    }

    if final_messages.is_empty() {
        let message = if uses_compaction_strategy(&context_strategy) {
            "No messages fit within the effective context window. Increase the context limit or reduce the pinned/latest message size and retry."
        } else {
            "No messages survived sliding-window FIFO selection. Increase the message window size and retry."
        };
        return Err(
            AgentRuntimeError::new(AgentRuntimeErrorType::EmptySelectionError, message)
                .with_code("EMPTY_MESSAGE_SELECTION"),
        );
    }

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
        .map_err(|e| {
            AgentRuntimeError::new(
                AgentRuntimeErrorType::AiServiceError,
                format!("Failed to emit LLM completion request: {}", e),
            )
            .with_code("EMIT_COMPLETION_REQUEST_FAILED")
        })?;

    log::info!("Emitted LLM completion request for session: {}", session_id);

    Ok(())
}

fn build_overflow_preflight(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
    safe_input_token_limit: usize,
) -> OverflowPreflight {
    let latest_message_tokens = messages
        .last()
        .map(crate::agent::llm::token_utils::estimate_tokens_bpe)
        .unwrap_or(0);
    let total_tokens = crate::agent::llm::token_utils::calculate_grounded_total_tokens(
        messages,
        system_prompt_tokens,
        tools_tokens,
    );
    let safety_margin =
        crate::agent::llm::token_utils::calculate_context_safety_margin(safe_input_token_limit);

    OverflowPreflight {
        latest_message_tokens,
        total_tokens,
        reserved_tokens: system_prompt_tokens + tools_tokens,
        safety_margin,
    }
}

async fn try_trigger_preflight_compaction(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
) -> Result<bool, String> {
    if messages.len() <= 1 {
        return Ok(false);
    }

    let split_idx = messages.len() - 1;
    if split_idx == 0 {
        return Ok(false);
    }
    if split_idx == 1
        && messages
            .first()
            .map(|message| message.id.starts_with("compact-summary-"))
            .unwrap_or(false)
    {
        return Ok(false);
    }

    let (compact_in_flight_arc, last_compacted_tail_id_arc, awaiting_compact_arc) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.compact_in_flight.clone(),
            session.last_compacted_tail_id.clone(),
            session.awaiting_compact_completion.clone(),
        )
    };

    let claimed_in_flight = compact_in_flight_arc
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    if !claimed_in_flight {
        awaiting_compact_arc.store(true, Ordering::SeqCst);
        let state_event = crate::agent::llm::types::CompactStateEvent {
            session_id: session_id.to_string(),
            session_name: Some(session_name.to_string()),
            compacting: true,
            phase: crate::agent::llm::types::CompactStatePhase::Started,
        };
        app_handle
            .emit("llm:compact-state", state_event)
            .map_err(|e| format!("Failed to emit llm:compact-state: {}", e))?;
        log::info!(
            "⏳ Reusing in-flight compaction and arming resume-after-compact: session={}",
            session_id
        );
        return Ok(true);
    }

    let current_tail_id = messages.last().map(|m| m.id.clone());
    let last_compacted_tail = last_compacted_tail_id_arc.read().await.clone();
    if current_tail_id.as_deref() == last_compacted_tail.as_deref() {
        compact_in_flight_arc.store(false, Ordering::SeqCst);
        return Ok(false);
    }

    let compact_msgs = messages[..split_idx].to_vec();
    let from_id = compact_msgs
        .first()
        .map(|m| m.id.clone())
        .unwrap_or_default();
    let to_id = compact_msgs
        .last()
        .map(|m| m.id.clone())
        .unwrap_or_default();

    awaiting_compact_arc.store(true, Ordering::SeqCst);
    *last_compacted_tail_id_arc.write().await = current_tail_id.clone();

    let compact_event = CompactRequest {
        session_id: session_id.to_string(),
        session_name: session_name.to_string(),
        messages: compact_msgs,
        from_id,
        to_id,
        resume_completion_after_compact: true,
    };
    let state_event = crate::agent::llm::types::CompactStateEvent {
        session_id: session_id.to_string(),
        session_name: Some(session_name.to_string()),
        compacting: true,
        phase: crate::agent::llm::types::CompactStatePhase::Started,
    };

    app_handle
        .emit("llm:compact-state", state_event)
        .map_err(|e| format!("Failed to emit llm:compact-state: {}", e))?;
    app_handle
        .emit("llm:compact-request", compact_event)
        .map_err(|e| format!("Failed to emit llm:compact-request: {}", e))?;

    Ok(true)
}

pub async fn trigger_preflight_compaction_for_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<bool, String> {
    let (session_name, messages) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let session_name = session
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| session_id.chars().take(8).collect::<String>());

        let messages = session
            .messages
            .read()
            .await
            .iter()
            .filter(|m| m.source.as_deref() != Some("recovery"))
            .cloned()
            .collect::<Vec<_>>();

        (session_name, messages)
    };

    let merged_messages = merge_consecutive_user_messages(messages);
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        session_id,
        &session_name,
        &merged_messages,
    )
    .await
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
