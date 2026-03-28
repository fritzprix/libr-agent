use crate::agent::references::build_default_registry;
use crate::agent::state::AgentSession;
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::CompactContextRepository;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::agent::llm::prompt::build_session_system_prompt_split;
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType, CompletionRequest};

use super::compaction::{
    find_preflight_compaction_split_index, should_trigger_background_compaction,
    trigger_background_compaction, try_trigger_preflight_compaction,
};
use super::context::{load_context_management_settings, uses_compaction_strategy};

#[derive(Debug)]
pub(crate) struct OverflowPreflight {
    pub(crate) preserved_tail_tokens: usize,
    pub(crate) total_tokens: usize,
    pub(crate) reserved_tokens: usize,
    pub(crate) safety_margin: usize,
    pub(crate) compactable_split_idx: usize,
}

pub(crate) fn build_overflow_preflight(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
    safe_input_token_limit: usize,
    compactable_split_idx: usize,
) -> OverflowPreflight {
    let preserved_tail_tokens = messages[compactable_split_idx.min(messages.len())..]
        .iter()
        .map(crate::agent::llm::token_utils::estimate_tokens_bpe)
        .sum();
    let total_tokens = crate::agent::llm::token_utils::calculate_grounded_total_tokens(
        messages,
        system_prompt_tokens,
        tools_tokens,
    );
    let safety_margin =
        crate::agent::llm::token_utils::calculate_context_safety_margin(safe_input_token_limit);

    OverflowPreflight {
        preserved_tail_tokens,
        total_tokens,
        reserved_tokens: system_prompt_tokens + tools_tokens,
        safety_margin,
        compactable_split_idx,
    }
}

const COMPACT_TOOL_SNAPSHOT_LIMIT: usize = 5;
const COMPACT_ARGUMENT_PREVIEW_LIMIT: usize = 96;
const COMPACT_RESULT_PREVIEW_LIMIT: usize = 140;

#[derive(Debug, Clone)]
struct CompactToolSnapshot {
    tool_name: String,
    argument_preview: String,
    status: &'static str,
    result_preview: String,
}

pub fn build_compact_summary_text(summary: &str, compacted_messages: &[Message]) -> String {
    let mut text = format!("### Previous Conversation Summary\n\n{}", summary.trim());
    let recent_tool_snapshot = summarize_recent_tool_calls(compacted_messages);

    if !recent_tool_snapshot.is_empty() {
        text.push_str("\n\n### Recent Tool Call Snapshot (latest 5)\n");
        text.push_str(
            &recent_tool_snapshot
                .into_iter()
                .map(|entry| format!("- {}", entry))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    text
}

fn summarize_recent_tool_calls(compacted_messages: &[Message]) -> Vec<String> {
    let tool_results_by_id: HashMap<&str, &Message> = compacted_messages
        .iter()
        .filter(|message| message.role == "tool")
        .filter_map(|message| message.tool_call_id.as_deref().map(|id| (id, message)))
        .collect();

    let mut snapshots = Vec::new();

    for message in compacted_messages {
        if message.role != "assistant" {
            continue;
        }

        let Some(tool_calls) = &message.tool_calls else {
            continue;
        };

        for tool_call in tool_calls {
            let Some(result_message) = tool_results_by_id.get(tool_call.id.as_str()) else {
                continue;
            };

            snapshots.push(CompactToolSnapshot {
                tool_name: tool_call.function.name.clone(),
                argument_preview: build_tool_argument_preview(&tool_call.function.arguments),
                status: determine_tool_result_status(result_message),
                result_preview: extract_tool_result_preview(result_message),
            });
        }
    }

    snapshots
        .into_iter()
        .rev()
        .take(COMPACT_TOOL_SNAPSHOT_LIMIT)
        .map(|snapshot| {
            format!(
                "{}({}) -> {}: {}",
                snapshot.tool_name,
                snapshot.argument_preview,
                snapshot.status,
                snapshot.result_preview,
            )
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn build_tool_argument_preview(arguments: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(arguments).ok();

    if let Some(serde_json::Value::Object(object)) = parsed {
        let mut preview_parts = Vec::new();
        let mut entries = object.iter().collect::<Vec<_>>();
        entries.sort_by(|(left_key, _), (right_key, _)| left_key.cmp(right_key));

        for (index, (key, value)) in entries.into_iter().enumerate() {
            if index >= 3 {
                preview_parts.push("...".to_string());
                break;
            }

            preview_parts.push(format!(
                "{}={}",
                key,
                truncate_for_compact_snapshot(&compact_json_value_preview(value), 32)
            ));
        }

        if preview_parts.is_empty() {
            "no-args".to_string()
        } else {
            truncate_for_compact_snapshot(&preview_parts.join(", "), COMPACT_ARGUMENT_PREVIEW_LIMIT)
        }
    } else if let Some(parsed_value) = parsed {
        truncate_for_compact_snapshot(&compact_json_value_preview(&parsed_value), 48)
    } else if arguments.trim().is_empty() {
        "no-args".to_string()
    } else {
        truncate_for_compact_snapshot(arguments.trim(), COMPACT_ARGUMENT_PREVIEW_LIMIT)
    }
}

fn compact_json_value_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(flag) => flag.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(values) => {
            if values.is_empty() {
                "[]".to_string()
            } else {
                format!("[{} item(s)]", values.len())
            }
        }
        serde_json::Value::Object(values) => {
            if values.is_empty() {
                "{}".to_string()
            } else {
                format!("{{{} key(s)}}", values.len())
            }
        }
    }
}

fn determine_tool_result_status(message: &Message) -> &'static str {
    if message.error.is_some() {
        return "error";
    }

    for content in &message.content {
        if let MCPContent::Text {
            is_error: Some(true),
            ..
        } = content
        {
            return "error";
        }
    }

    "success"
}

fn extract_tool_result_preview(message: &Message) -> String {
    let mut text_parts = Vec::new();

    for content in &message.content {
        match content {
            MCPContent::Text { text, .. } => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    text_parts.push(trimmed.to_string());
                }
            }
            MCPContent::Resource { resource, .. } => {
                let mime_type = resource
                    .get("mimeType")
                    .and_then(|value| value.as_str())
                    .unwrap_or("resource");
                text_parts.push(format!("[resource:{}]", mime_type));
            }
            MCPContent::Image { .. } => text_parts.push("[image output]".to_string()),
            MCPContent::Audio { .. } => text_parts.push("[audio output]".to_string()),
            MCPContent::Thinking { .. } => {}
            MCPContent::ToolCall { name, .. } => {
                text_parts.push(format!("[tool call:{}]", name));
            }
        }
    }

    if text_parts.is_empty() {
        "completed with no textual result".to_string()
    } else {
        truncate_for_compact_snapshot(
            &text_parts.join(" | ").replace('\n', " "),
            COMPACT_RESULT_PREVIEW_LIMIT,
        )
    }
}

fn truncate_for_compact_snapshot(value: &str, max_len: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_len {
        return normalized;
    }

    let truncated = normalized
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    format!("{}…", truncated)
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
    let raw_message_count = messages.len();
    let merged_messages = merge_consecutive_user_messages(messages);
    let merged_message_count = merged_messages.len();
    let messages =
        crate::agent::llm::context_selector::remove_incomplete_tool_chains(merged_messages);
    let cleaned_message_count = messages.len();

    log::info!(
        "🧱 Prompt message normalization: session={}, raw={}, merged={}, cleaned={}",
        session_id,
        raw_message_count,
        merged_message_count,
        cleaned_message_count
    );

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

    let (messages, compact_summary_injected) = {
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
                (messages, false)
            } else if let Some(to_idx) = messages.iter().position(|m| m.id == record.to_id) {
                let now_ms = chrono::Utc::now().timestamp_millis();
                let summary_msg = Message {
                    id: format!("compact-summary-{}", session_id),
                    session_id: session_id.clone(),
                    role: "user".to_string(),
                    content: vec![MCPContent::Text {
                        text: build_compact_summary_text(&record.summary, &messages[..=to_idx]),
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
                ([vec![summary_msg], tail].concat(), true)
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
                (messages, false)
            }
        } else {
            (messages, false)
        }
    };

    log::info!(
        "🧱 Prompt prefix composition: session={}, compact_summary_injected={}, message_count={}",
        session_id,
        compact_summary_injected,
        messages.len()
    );

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

        let compactable_split_idx = find_preflight_compaction_split_index(&messages);
        let preflight = build_overflow_preflight(
            &messages,
            system_prompt_tokens + session_context_tokens,
            tools_tokens,
            safe_input_token_limit,
            compactable_split_idx,
        );

        let preserved_tail_projected_tokens =
            preflight.reserved_tokens + preflight.preserved_tail_tokens + preflight.safety_margin;
        if preserved_tail_projected_tokens > safe_input_token_limit {
            let mut context = serde_json::Map::new();
            context.insert(
                "projectedTokens".to_string(),
                serde_json::json!(preserved_tail_projected_tokens),
            );
            context.insert(
                "effectiveLimit".to_string(),
                serde_json::json!(safe_input_token_limit),
            );
            context.insert(
                "compactableSplitIndex".to_string(),
                serde_json::json!(preflight.compactable_split_idx),
            );
            return Err(
                AgentRuntimeError::new(
                    AgentRuntimeErrorType::ContextLimitError,
                    format!(
                        "The newest non-compactable context is too large for the configured context window (projected {} > limit {}). Reduce the newest message or attachment payload and retry.",
                        preserved_tail_projected_tokens, safe_input_token_limit
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

    // 4. Generate response message ID and store in session for matching
    let response_message_id = cuid2::create_id();
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            let mut expected_id = session.expected_response_id.write().await;
            *expected_id = Some(response_message_id.clone());
        }
    }

    let request = CompletionRequest {
        session_id: session_id.clone(),
        response_message_id,
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

/// Resolve `@type:arg` references in user messages.
/// Each user message's text content is processed through the reference registry.
/// Only the returned `Vec<Message>` is modified — the session store is untouched.
pub(crate) async fn resolve_message_references(
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
pub fn merge_consecutive_user_messages(messages: Vec<Message>) -> Vec<Message> {
    if messages.len() < 2 {
        return messages;
    }

    let trailing_run_start = messages
        .iter()
        .rposition(|msg| msg.role != "user")
        .map(|idx| idx + 1)
        .unwrap_or(0);

    if trailing_run_start >= messages.len().saturating_sub(1) {
        return messages;
    }

    let (head, trailing_users) = messages.split_at(trailing_run_start);
    if trailing_users.iter().any(|msg| msg.role != "user") {
        return messages;
    }

    let mut result: Vec<Message> = head.to_vec();
    let mut merged = trailing_users[0].clone();

    for msg in trailing_users.iter().skip(1) {
        merged.content.push(MCPContent::Text {
            text: "\n\n---\n\n".to_string(),
            is_error: None,
        });
        merged.content.extend(msg.content.clone());
        log::info!(
            "Merged trailing consecutive user messages: base={}, appended={}",
            merged.id,
            msg.id
        );
    }

    result.push(merged);
    result
}
