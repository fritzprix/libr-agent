use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::agent::llm::completion::context::{
    load_context_management_settings, uses_compaction_strategy, ContextManagementSettings,
};
use crate::agent::llm::prompt::build_session_system_prompt_split;
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactionParentRequest, CompletionRequest,
};

use super::compact::build_compact_summary_message_for_messages;
use super::context_selection::{
    resolve_preserved_calibration_ratio, trigger_preflight_compaction_for_messages_or_error,
};
use super::formatting::{merge_consecutive_user_messages, resolve_message_references};

struct SessionReadSnapshot {
    session_name: String,
    model: String,
    provider: String,
    agent_config: crate::agent::AgentConfig,
    messages: Vec<Message>,
}

struct PromptTokenCounts {
    system_prompt_tokens: usize,
    tools_tokens: usize,
}

/// Request LLM completion from frontend
///
/// Note: session_repo is passed through to handle_llm_response which uses it for status updates
pub async fn request_llm_completion(
    _session_repo: &Arc<dyn crate::repositories::SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), AgentRuntimeError> {
    // 1. Validate session status before mutating pending state
    validate_session_status(active_sessions, &session_id).await?;

    // 2. Drain pending messages only after the session is still eligible to run
    process_pending_messages(active_sessions, app_handle, &session_id).await?;

    // 3. Snapshot session config/messages after pending messages have been integrated
    let snapshot = snapshot_session(active_sessions, &session_id).await?;

    // 4. Build system prompt
    let (system_prompt, session_context) =
        build_system_prompt(active_sessions, proxy_manager, &session_id).await?;

    // 5. Collect tools
    let available_tools = crate::agent::tools::collect_available_tools(&session_id, proxy_manager)
        .await
        .ok();
    let tools_json = available_tools
        .as_ref()
        .map(|tools| serde_json::to_string(tools).unwrap_or_default());

    // 6. Message Normalization (References & Merging)
    let messages = resolve_message_references(
        snapshot.messages,
        &session_id,
        snapshot.agent_config.id.as_deref(),
    )
    .await;
    let normalized_messages = normalize_messages(messages, &session_id);

    // 7. Context Settings & Tokens
    let context_settings = load_context_management_settings().await;
    let raw_messages = normalized_messages.clone();
    let measured_output_tokens_reserve =
        crate::agent::llm::token_utils::derive_measured_output_tokens_reserve(
            &normalized_messages,
            snapshot.agent_config.max_tokens,
        );

    // 8. Inject Compact Summary
    let (messages_with_summary, compact_summary_injected) = inject_compact_summary(
        active_sessions,
        &session_id,
        normalized_messages.clone(),
        &context_settings.context_strategy,
    )
    .await?;

    // 9. Select Final Messages
    let final_messages =
        select_final_messages(messages_with_summary, &snapshot.provider, &context_settings);

    let compaction_parent_request = Some(CompactionParentRequest {
        model: snapshot.model.clone(),
        provider: snapshot.provider.clone(),
        system_prompt: system_prompt.clone(),
        session_context: session_context.clone(),
        available_tools: available_tools.clone(),
    });

    store_last_completion_request(active_sessions, &session_id, &compaction_parent_request).await;

    // 10. Check empty messages
    match check_empty_messages(
        &final_messages,
        active_sessions,
        app_handle,
        &session_id,
        &snapshot.session_name,
        &context_settings,
        &normalized_messages,
        &compaction_parent_request,
        measured_output_tokens_reserve,
    )
    .await
    {
        Ok(()) => {}
        Err(e) => {
            if e.details.as_ref().and_then(|d| d.error_code.as_deref())
                == Some("COMPACTION_TRIGGERED_OK")
            {
                return Ok(());
            }
            return Err(e);
        }
    }

    let request_layout = crate::agent::llm::build_request_layout(
        &snapshot.provider,
        &session_id,
        system_prompt,
        session_context,
        final_messages,
    );
    let token_counts = compute_prompt_tokens(&request_layout.system_prompt, &tools_json);
    let system_prompt_tokens = token_counts.system_prompt_tokens;
    let tools_tokens = token_counts.tools_tokens;
    let preserved_calibration_ratio = resolve_preserved_calibration_ratio(
        &raw_messages,
        &request_layout.messages,
        system_prompt_tokens,
        tools_tokens,
        &snapshot.provider,
    );

    // 11. Check token limit and trigger preflight compaction or fail explicitly
    let final_messages = match check_token_limit(
        request_layout.messages,
        active_sessions,
        app_handle,
        &session_id,
        &snapshot.session_name,
        &context_settings,
        system_prompt_tokens,
        tools_tokens,
        preserved_calibration_ratio,
        compact_summary_injected,
        &normalized_messages,
        compaction_parent_request,
        measured_output_tokens_reserve,
    )
    .await
    {
        Ok(msgs) => msgs,
        Err(e) => {
            if e.details.as_ref().and_then(|d| d.error_code.as_deref())
                == Some("COMPACTION_TRIGGERED_OK")
            {
                return Ok(());
            }
            return Err(e);
        }
    };

    // 12. Generate ID & Emit Request
    let response_message_id = generate_response_message_id(active_sessions, &session_id).await;

    let request = CompletionRequest {
        session_id: session_id.clone(),
        response_message_id,
        messages: final_messages,
        model: snapshot.model,
        provider: snapshot.provider,
        system_prompt: request_layout.system_prompt,
        temperature: snapshot.agent_config.temperature,
        max_tokens: snapshot.agent_config.max_tokens,
        available_tools,
    };

    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            *session.last_submitted_input_message_id.write().await =
                crate::agent::llm::select_last_submitted_input_message_id(&request.messages);
        }
    }

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

// ============================================================================
// Section: Orchestration Helpers
// ============================================================================

async fn process_pending_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<(), AgentRuntimeError> {
    let pending_messages: Vec<Message> = {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let mut pending_events = session.pending_events.write().await;
            let pending_ids = pending_events.drain_messages();

            if pending_ids.is_empty() {
                Vec::new()
            } else {
                let repo = crate::state::get_message_repository();
                match repo.get_by_ids(pending_ids).await {
                    Ok(msgs) => {
                        log::info!(
                            "Drained {} pending messages from queue for session {}",
                            msgs.len(),
                            session_id
                        );
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
    };

    for msg in pending_messages {
        let event = crate::agent::events::AgentEvent::MessageAdded {
            session_id: session_id.to_string(),
            message: Box::new(msg.clone()),
        };
        let _ = crate::agent::tauri_events::emit_agent_event(app_handle, event);
    }

    Ok(())
}

async fn validate_session_status(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(), AgentRuntimeError> {
    let sessions = active_sessions.read().await;
    let session = sessions.get(session_id).ok_or_else(|| {
        AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            format!("Session not found: {}", session_id),
        )
        .with_code("SESSION_NOT_FOUND")
    })?;

    if session.cancel_pending.load(Ordering::SeqCst)
        || session.cancellation_token.is_cancelled()
        || session.metadata.status != SessionStatus::Busy
    {
        log::info!(
            "Rejecting LLM request for session {} (status: {:?}, cancel_pending={}, is_cancelled={})",
            session_id,
            session.metadata.status,
            session.cancel_pending.load(Ordering::SeqCst),
            session.cancellation_token.is_cancelled()
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

    Ok(())
}

async fn snapshot_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<SessionReadSnapshot, AgentRuntimeError> {
    let sessions = active_sessions.read().await;
    let session = sessions.get(session_id).ok_or_else(|| {
        AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            format!("Session not found: {}", session_id),
        )
        .with_code("SESSION_NOT_FOUND")
    })?;

    let agent_config = crate::agent::resolve_agent_config(&session.metadata)
        .await
        .map_err(|e| {
            AgentRuntimeError::new(
                AgentRuntimeErrorType::AiServiceError,
                format!("Invalid agent config: {}", e),
            )
            .with_code("INVALID_AGENT_CONFIG")
        })?;

    let messages = session
        .messages
        .read()
        .await
        .iter()
        .filter(|m| !m.is_recovery_message())
        .cloned()
        .collect::<Vec<_>>();

    let session_name = session
        .metadata
        .name
        .clone()
        .unwrap_or_else(|| session_id.chars().take(8).collect::<String>());

    log::info!(
        "🔄 Message stack for LLM request: session={}, count={}, first_msg_id={}, last_msg_id={}",
        session_id,
        messages.len(),
        messages.first().map(|m| m.id.as_str()).unwrap_or("none"),
        messages.last().map(|m| m.id.as_str()).unwrap_or("none")
    );

    Ok(SessionReadSnapshot {
        session_name,
        model: session.metadata.model.clone(),
        provider: session.metadata.provider.clone(),
        agent_config,
        messages,
    })
}

async fn build_system_prompt(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    session_id: &str,
) -> Result<(Option<String>, Option<String>), AgentRuntimeError> {
    let (stable_prompt, session_context) =
        build_session_system_prompt_split(active_sessions, proxy_manager, session_id).await?;
    Ok((Some(stable_prompt), session_context))
}

fn normalize_messages(messages: Vec<Message>, session_id: &str) -> Vec<Message> {
    let raw_message_count = messages.len();
    let merged_messages = merge_consecutive_user_messages(messages);
    let merged_message_count = merged_messages.len();
    let cleaned =
        crate::agent::llm::context_selector::remove_incomplete_tool_chains(merged_messages);

    log::info!(
        "🧱 Prompt message normalization: session={}, raw={}, merged={}, cleaned={}",
        session_id,
        raw_message_count,
        merged_message_count,
        cleaned.len()
    );
    cleaned
}

fn compute_prompt_tokens(
    system_prompt: &Option<String>,
    tools_json: &Option<String>,
) -> PromptTokenCounts {
    let system_prompt_tokens = system_prompt
        .as_ref()
        .map(|prompt| crate::agent::llm::token_utils::estimate_text_tokens(prompt))
        .unwrap_or(0);

    let tools_tokens = tools_json
        .as_ref()
        .map(|json| crate::agent::llm::token_utils::estimate_text_tokens(json))
        .unwrap_or(0);

    PromptTokenCounts {
        system_prompt_tokens,
        tools_tokens,
    }
}

async fn inject_compact_summary(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    messages: Vec<Message>,
    context_strategy: &str,
) -> Result<(Vec<Message>, bool), AgentRuntimeError> {
    let compact_context_arc = {
        let active = active_sessions.read().await;
        let session = active.get(session_id).ok_or_else(|| {
            AgentRuntimeError::new(
                AgentRuntimeErrorType::AiServiceError,
                format!("Session not found: {}", session_id),
            )
            .with_code("SESSION_NOT_FOUND")
        })?;
        session.compact_context.clone()
    };

    let compact_record = compact_context_arc.read().await.clone();
    if let Some(record) = compact_record {
        if !uses_compaction_strategy(context_strategy) {
            *compact_context_arc.write().await = None;
            let compact_repo = crate::state::get_compact_context_repository();
            if let Err(e) = compact_repo.delete_by_session_id(session_id).await {
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
            Ok((messages, false))
        } else if let Some(to_idx) = messages.iter().position(|m| m.id == record.to_id) {
            let now_ms = chrono::Utc::now().timestamp_millis();
            let summary_msg = build_compact_summary_message_for_messages(
                session_id,
                &record.summary,
                &messages[..=to_idx],
                now_ms,
            );
            let summary_tokens = crate::agent::llm::token_utils::estimate_tokens_bpe(&summary_msg);
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
            Ok(([vec![summary_msg], tail].concat(), true))
        } else {
            *compact_context_arc.write().await = None;
            let compact_repo = crate::state::get_compact_context_repository();
            if let Err(e) = compact_repo.delete_by_session_id(session_id).await {
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
            Ok((messages, false))
        }
    } else {
        Ok((messages, false))
    }
}

fn select_final_messages(
    messages: Vec<Message>,
    provider: &str,
    context_settings: &ContextManagementSettings,
) -> Vec<Message> {
    if uses_compaction_strategy(&context_settings.context_strategy) {
        messages
    } else {
        crate::agent::llm::context_selector::select_recent_messages_fifo(
            &messages,
            provider,
            context_settings.window_size,
            if provider == "gemini" {
                100
            } else {
                context_settings.tool_call_group_visible_count
            },
        )
    }
}

async fn store_last_completion_request(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    request: &Option<CompactionParentRequest>,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        let mut last_completion_request = session.last_completion_request.write().await;
        *last_completion_request = request.clone();
    }
}

#[allow(clippy::too_many_arguments)]
async fn check_empty_messages(
    final_messages: &[Message],
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    context_settings: &ContextManagementSettings,
    normalized_messages: &[Message],
    compaction_parent_request: &Option<CompactionParentRequest>,
    measured_output_tokens_reserve: usize,
) -> Result<(), AgentRuntimeError> {
    if !final_messages.is_empty() {
        return Ok(());
    }

    if uses_compaction_strategy(&context_settings.context_strategy)
        && trigger_preflight_compaction_for_messages_or_error(
            active_sessions,
            app_handle,
            session_id,
            session_name,
            normalized_messages,
            compaction_parent_request.clone(),
            measured_output_tokens_reserve,
        )
        .await?
    {
        return Err(AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            "Compaction triggered",
        )
        .with_code("COMPACTION_TRIGGERED_OK"));
    }

    let message = if uses_compaction_strategy(&context_settings.context_strategy) {
        "No messages fit within the effective context window. Increase the context limit or reduce the pinned/latest message size and retry."
    } else {
        "No messages survived sliding-window FIFO selection. Increase the message window size and retry."
    };

    Err(
        AgentRuntimeError::new(AgentRuntimeErrorType::EmptySelectionError, message)
            .with_code("EMPTY_MESSAGE_SELECTION"),
    )
}

fn find_lossy_fallback_split_index(messages: &[Message], compaction_limit: usize) -> Option<usize> {
    if compaction_limit == 0 {
        return None;
    }
    let message_tokens = messages
        .iter()
        .map(crate::agent::llm::token_utils::estimate_tokens_bpe)
        .collect::<Vec<_>>();
    let total_message_tokens: usize = message_tokens.iter().sum();
    if total_message_tokens < compaction_limit {
        return Some(0);
    }

    let start_search_idx = if messages
        .first()
        .map(|m| m.is_compact_summary())
        .unwrap_or(false)
    {
        1
    } else {
        0
    };

    let mut current_suffix_sum: usize = message_tokens[start_search_idx..].iter().sum();

    for split_idx in start_search_idx..messages.len() {
        if current_suffix_sum < compaction_limit {
            return Some(split_idx);
        }
        if split_idx < message_tokens.len() {
            current_suffix_sum = current_suffix_sum.saturating_sub(message_tokens[split_idx]);
        }
    }
    None
}

fn build_lossy_fallback_drop_slice(messages: &[Message], split_idx: usize) -> &[Message] {
    let start_idx = usize::from(
        messages
            .first()
            .map(|message| message.is_compact_summary())
            .unwrap_or(false),
    );
    let clamped_end = split_idx.clamp(start_idx, messages.len());
    &messages[start_idx..clamped_end]
}

async fn perform_lossy_fallback_drop(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    messages_to_drop: &[Message],
    final_messages: &mut Vec<Message>,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    if messages_to_drop.is_empty() {
        return Ok(());
    }

    let dropped_ids: std::collections::HashSet<String> =
        messages_to_drop.iter().map(|m| m.id.clone()).collect();

    // 1. Retrieve the messages handle under a short-lived read lock to avoid holding active_sessions lock across write
    let messages_handle = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.messages.clone())
    };

    // 2. Perform in-memory mutations first
    if let Some(messages_handle) = &messages_handle {
        let mut session_messages = messages_handle.write().await;
        session_messages.retain(|m| !dropped_ids.contains(&m.id));
    }

    final_messages.retain(|m| m.is_compact_summary() || !dropped_ids.contains(&m.id));

    // Create and insert a persistent system trim notice message
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let trim_notice = Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "system".to_string(),
        content: vec![crate::mcp::types::MCPContent::Text {
            text: format!(
                "⚠️ Trimmed {} older messages to fit the context limit.",
                messages_to_drop.len()
            ),
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at: now,
        updated_at: now,
        source: None,
        error: None,
        metadata: None,
    };

    // Add to in-memory session cache
    if let Some(messages_handle) = &messages_handle {
        let mut session_messages = messages_handle.write().await;
        session_messages.push(trim_notice.clone());
    }

    // 3. Delete old messages from SQLite database and insert the trim notice
    // (This is run after memory mutations to prevent DB deleted vs memory present inconsistency)
    let repo = crate::state::get_message_repository();
    for msg in messages_to_drop {
        if let Err(error) = repo.delete_by_id(&msg.id).await {
            log::error!(
                "Failed to delete trimmed message {} from SQLite: {}",
                msg.id,
                error
            );
        }
    }
    if let Err(e) = repo.insert(&trim_notice).await {
        log::error!("Failed to insert trim notice to SQLite: {}", e);
    }

    // 4. Emit the notification events to UI
    let event_trim = crate::agent::events::AgentEvent::ContextMessagesTrimmed {
        session_id: session_id.to_string(),
        dropped_count: messages_to_drop.len(),
    };
    let _ = crate::agent::tauri_events::emit_agent_event(app_handle, event_trim);

    let event_msg = crate::agent::events::AgentEvent::MessageAdded {
        session_id: session_id.to_string(),
        message: Box::new(trim_notice),
    };
    let _ = crate::agent::tauri_events::emit_agent_event(app_handle, event_msg);

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn check_token_limit(
    final_messages: Vec<Message>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    context_settings: &ContextManagementSettings,
    system_prompt_tokens: usize,
    tools_tokens: usize,
    preserved_calibration_ratio: Option<f64>,
    compact_summary_injected: bool,
    normalized_messages: &[Message],
    compaction_parent_request: Option<CompactionParentRequest>,
    measured_output_tokens_reserve: usize,
) -> Result<Vec<Message>, AgentRuntimeError> {
    if !uses_compaction_strategy(&context_settings.context_strategy) {
        return Ok(final_messages);
    }

    let safe_input_token_limit = std::cmp::min(
        context_settings.max_input_context,
        context_settings.model_max_limit,
    );
    let prompt_anchored_total_tokens =
        crate::agent::llm::token_utils::calculate_prompt_anchored_total_tokens(
            &final_messages,
            system_prompt_tokens,
            tools_tokens,
        );
    let conservative_preflight_tokens =
        crate::agent::llm::token_utils::calculate_conservative_preflight_prompt_tokens(
            &final_messages,
            system_prompt_tokens,
            tools_tokens,
            preserved_calibration_ratio,
        );
    let effective_input_budget = crate::agent::llm::token_utils::calculate_effective_input_budget(
        safe_input_token_limit,
        measured_output_tokens_reserve,
    );
    let total_budget_tokens =
        conservative_preflight_tokens.saturating_add(measured_output_tokens_reserve);
    let preflight_metrics = crate::agent::events::PreflightTokenMetrics {
        conservative_prompt_tokens: conservative_preflight_tokens,
        prompt_anchored_total_tokens,
        safe_input_token_limit,
        measured_output_tokens_reserve,
        effective_input_budget,
        total_budget_tokens,
        system_prompt_tokens,
        tools_tokens,
        selected_message_count: final_messages.len(),
        compact_summary_injected,
        preserved_calibration_ratio,
    };
    let event = crate::agent::events::AgentEvent::PreflightTokenMetricsUpdated {
        session_id: session_id.to_string(),
        metrics: preflight_metrics,
    };
    let _ = crate::agent::tauri_events::emit_agent_event(app_handle, event);

    if total_budget_tokens < safe_input_token_limit {
        return Ok(final_messages);
    }

    let selected_message_breakdown =
        crate::agent::llm::token_utils::summarize_message_token_breakdown(&final_messages);
    log::info!(
        "⛔ Rust preflight blocked LLM request: session={}, conservative_prompt_tokens={}, measured_output_tokens_reserve={}, total_budget_tokens={}, effective_input_budget={}, safe_input_token_limit={}, compact_summary_injected={}, selected_message_count={}, system_prompt_tokens={}, tools_tokens={}, preserved_calibration_ratio={}, selected_breakdown=[{}]",
        session_id,
        conservative_preflight_tokens,
        measured_output_tokens_reserve,
        total_budget_tokens,
        effective_input_budget,
        safe_input_token_limit,
        compact_summary_injected,
        final_messages.len(),
        system_prompt_tokens,
        tools_tokens,
        preserved_calibration_ratio
            .map(|ratio| format!("{:.4}", ratio))
            .unwrap_or_else(|| "none".to_string()),
        selected_message_breakdown
    );

    let compact_context_record = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compact_context.clone())
    };
    let compact_context_record = match compact_context_record {
        Some(handle) => handle.read().await.clone(),
        None => None,
    };

    let compaction_limit = effective_input_budget
        .saturating_sub(system_prompt_tokens)
        .saturating_sub(tools_tokens)
        .saturating_sub(1500); // 1500 tokens for summary safety margin

    let mut final_messages = final_messages;

    if !crate::agent::llm::completion::compaction::has_prompt_checkpoint_compaction_target(
        normalized_messages,
        compact_context_record.as_ref(),
        compaction_limit,
    ) {
        log::warn!(
            "⚠️ No prompt-token checkpoint found for compaction. Performing lossy fallback by dropping older messages: session={}",
            session_id
        );

        if let Some(split_idx) =
            find_lossy_fallback_split_index(normalized_messages, compaction_limit)
        {
            let messages_to_drop = build_lossy_fallback_drop_slice(normalized_messages, split_idx);
            if !messages_to_drop.is_empty() {
                if let Err(err) = perform_lossy_fallback_drop(
                    active_sessions,
                    session_id,
                    messages_to_drop,
                    &mut final_messages,
                    app_handle,
                )
                .await
                {
                    log::error!(
                        "Failed to drop messages during lossy fallback: session={}, error={}",
                        session_id,
                        err
                    );
                } else {
                    return Ok(final_messages);
                }
            }
        }

        return Err(
            AgentRuntimeError::new(
                AgentRuntimeErrorType::ContextLimitError,
                "Prepared payload exceeds the effective context limit, but there is no persisted prompt-token checkpoint to compact from, and lossy fallback failed to split.",
            )
            .with_code("INVALID_CONTEXT_STATE")
            .with_original_error(serde_json::json!({
                "sessionId": session_id,
                "conservativePromptTokens": conservative_preflight_tokens,
                "measuredOutputTokensReserve": measured_output_tokens_reserve,
                "totalBudgetTokens": total_budget_tokens,
                "effectiveInputBudget": effective_input_budget,
                "safeInputTokenLimit": safe_input_token_limit,
                "compactSummaryInjected": compact_summary_injected,
                "selectedMessageCount": final_messages.len(),
                "systemPromptTokens": system_prompt_tokens,
                "toolsTokens": tools_tokens,
                "preservedCalibrationRatio": preserved_calibration_ratio,
                "selectedBreakdown": selected_message_breakdown,
                "lossyFallbackAttempted": true,
            })),
        );
    }

    if trigger_preflight_compaction_for_messages_or_error(
        active_sessions,
        app_handle,
        session_id,
        session_name,
        normalized_messages,
        compaction_parent_request.clone(),
        measured_output_tokens_reserve,
    )
    .await?
    {
        return Err(AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            "Compaction triggered",
        )
        .with_code("COMPACTION_TRIGGERED_OK"));
    }

    // If compaction was skipped or failed to fit, perform lossy fallback
    log::warn!(
        "⚠️ Compaction skipped or could not reduce context. Performing lossy fallback by dropping older messages: session={}",
        session_id
    );

    if let Some(split_idx) = find_lossy_fallback_split_index(normalized_messages, compaction_limit)
    {
        let messages_to_drop = build_lossy_fallback_drop_slice(normalized_messages, split_idx);
        if !messages_to_drop.is_empty() {
            if let Err(err) = perform_lossy_fallback_drop(
                active_sessions,
                session_id,
                messages_to_drop,
                &mut final_messages,
                app_handle,
            )
            .await
            {
                log::error!(
                    "Failed to drop messages during compaction skipped lossy fallback: session={}, error={}",
                    session_id,
                    err
                );
            } else {
                return Ok(final_messages);
            }
        }
    }

    Err(
        AgentRuntimeError::new(
            AgentRuntimeErrorType::ContextLimitError,
            format!(
                "Prepared payload exceeds the effective context limit before send ({} total budget tokens >= {} input limit; conservative input={}, reserved output={}), and lossy fallback could not find a valid message split.",
                total_budget_tokens,
                safe_input_token_limit,
                conservative_preflight_tokens,
                measured_output_tokens_reserve
            ),
        )
        .with_code("RUST_PREFLIGHT_CONTEXT_LIMIT")
        .with_original_error(serde_json::json!({
            "sessionId": session_id,
            "conservativePromptTokens": conservative_preflight_tokens,
            "measuredOutputTokensReserve": measured_output_tokens_reserve,
            "totalBudgetTokens": total_budget_tokens,
            "effectiveInputBudget": effective_input_budget,
            "safeInputTokenLimit": safe_input_token_limit,
            "compactSummaryInjected": compact_summary_injected,
            "selectedMessageCount": final_messages.len(),
            "systemPromptTokens": system_prompt_tokens,
            "toolsTokens": tools_tokens,
            "preservedCalibrationRatio": preserved_calibration_ratio,
            "selectedBreakdown": selected_message_breakdown,
            "lossyFallbackAttempted": true,
        })),
    )
}

async fn generate_response_message_id(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> String {
    let response_message_id = cuid2::create_id();
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        let mut expected_id = session.expected_response_id.write().await;
        *expected_id = Some(response_message_id.clone());
    }
    response_message_id
}
