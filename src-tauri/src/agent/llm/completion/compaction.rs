use crate::agent::state::AgentSession;
use crate::agent::state::DeferredWorkflowStep;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::CompactContextRecord;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use super::context::{load_context_management_settings, uses_compaction_strategy};
use crate::agent::llm::types::{CompactRequest, CompactionParentRequest};

pub fn should_trigger_background_compaction(
    current_tokens: usize,
    safe_input_token_limit: usize,
    context_strategy: &str,
) -> bool {
    uses_compaction_strategy(context_strategy)
        && current_tokens
            > crate::agent::llm::token_utils::calculate_compact_threshold(safe_input_token_limit)
}

async fn resolve_parent_request(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    explicit_parent_request: Option<CompactionParentRequest>,
) -> Option<CompactionParentRequest> {
    if explicit_parent_request.is_some() {
        return explicit_parent_request;
    }

    let request_handle = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.last_completion_request.clone())
    }?;

    let request = request_handle.read().await.clone();
    request
}

pub(crate) struct BackgroundCompactionHandles {
    pub(crate) compact_in_flight_arc: Arc<AtomicBool>,
    pub(crate) last_compacted_tail_id_arc: Arc<RwLock<Option<String>>>,
}

fn build_incremental_compact_summary_message(
    session_id: &str,
    summary: &str,
    created_at: i64,
) -> Message {
    Message {
        id: format!("compact-summary-{}", session_id),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: super::request::build_compact_summary_text(summary, &[]),
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
        created_at,
        updated_at: created_at,
        source: Some("compact-summary".to_string()),
        error: None,
        metadata: None,
    }
}

fn build_compaction_request_payload(
    session_id: &str,
    messages: &[Message],
    split_idx: usize,
    compact_record: Option<&CompactContextRecord>,
    created_at: i64,
) -> Option<(Vec<Message>, String, String, usize, bool)> {
    if split_idx == 0 {
        return None;
    }

    if let Some(record) = compact_record {
        if let Some(compacted_to_idx) = messages
            .iter()
            .position(|message| message.id == record.to_id)
        {
            // Incremental compaction input is:
            //   [previous summary as one synthetic message] + [raw delta since record.to_id]
            // It is deliberately not the full compactable prefix.
            let first_delta_message_idx = compacted_to_idx.saturating_add(1);
            if first_delta_message_idx >= split_idx {
                return None;
            }

            let mut compact_messages = Vec::with_capacity(1 + split_idx - first_delta_message_idx);
            compact_messages.push(build_incremental_compact_summary_message(
                session_id,
                &record.summary,
                created_at,
            ));
            compact_messages.extend(messages[first_delta_message_idx..split_idx].iter().cloned());

            return Some((
                compact_messages,
                record.from_id.clone(),
                messages[split_idx - 1].id.clone(),
                split_idx - first_delta_message_idx,
                true,
            ));
        }
    }

    // First compaction in a session has no previous compact summary yet, so the
    // compactable raw prefix becomes the initial delta baseline.
    let compact_messages = messages[..split_idx].to_vec();
    Some((
        compact_messages,
        messages
            .first()
            .map(|message| message.id.clone())
            .unwrap_or_default(),
        messages[split_idx - 1].id.clone(),
        split_idx,
        false,
    ))
}

/// Determines the compactable prefix for preflight compaction.
///
/// Unlike background compaction, preflight compaction must preserve the newest
/// direct user turn so we don't summarize away the input that still needs an
/// answer. However, workflow-generated tool output at the tail is compactable,
/// and unresolved tool chains must remain intact. This helper aligns those
/// constraints into a single split index:
///
/// - latest `user`/non-tool tail: preserve the last message
/// - latest `tool` tail: compact as much as `find_compaction_split_index()` allows
/// - unresolved tool chain: preserve the full unresolved tail
pub fn find_preflight_compaction_split_index(messages: &[Message]) -> usize {
    if messages.is_empty() {
        return 0;
    }

    let unresolved_boundary =
        crate::agent::llm::context_selector::find_compaction_split_index(messages);

    match messages.last().map(|message| message.role.as_str()) {
        Some("tool") => unresolved_boundary,
        Some(_) => std::cmp::min(messages.len().saturating_sub(1), unresolved_boundary),
        None => 0,
    }
}

pub fn should_skip_same_tail_compaction(messages: &[Message], split_idx: usize) -> bool {
    let has_compact_summary = messages
        .first()
        .map(|message| message.id.starts_with("compact-summary-"))
        .unwrap_or(false);

    if !has_compact_summary {
        // Without a persisted compact summary at the head, a same-tail retry is
        // still meaningful: the previous compaction may have failed or been
        // abandoned before Rust stored the summary record.
        return false;
    }

    split_idx <= 1
}

pub(crate) async fn trigger_background_compaction(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    parent_request: Option<CompactionParentRequest>,
    handles: &BackgroundCompactionHandles,
) -> Result<bool, String> {
    let split_idx = crate::agent::llm::context_selector::find_compaction_split_index(messages);
    if split_idx == 0 {
        return Ok(false);
    }

    let claimed_in_flight = handles
        .compact_in_flight_arc
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();

    if !claimed_in_flight {
        log::debug!("⏭️ Compaction skipped (in_flight): session={}", session_id);
        return Ok(false);
    }

    let current_tail_id = messages.last().map(|m| m.id.clone());
    let last_compacted_tail = handles.last_compacted_tail_id_arc.read().await.clone();
    let same_tail = current_tail_id.as_deref() == last_compacted_tail.as_deref();

    if same_tail && should_skip_same_tail_compaction(messages, split_idx) {
        handles.compact_in_flight_arc.store(false, Ordering::SeqCst);
        log::debug!(
            "⏭️ Compaction skipped (same tail): session={}, tail={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?")
        );
        return Ok(false);
    }
    let started_at_ms = chrono::Utc::now().timestamp_millis();
    let compact_context_record = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compact_context.clone())
    };
    let compact_context_record = if let Some(compact_context_handle) = compact_context_record {
        compact_context_handle.read().await.clone()
    } else {
        None
    };
    let Some((compact_msgs, from_id, to_id, compacted_delta_count, reused_prior_summary)) =
        build_compaction_request_payload(
            session_id,
            messages,
            split_idx,
            compact_context_record.as_ref(),
            started_at_ms,
        )
    else {
        handles.compact_in_flight_arc.store(false, Ordering::SeqCst);
        log::debug!(
            "⏭️ Compaction skipped (no new delta beyond previous summary): session={}, tail={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?")
        );
        return Ok(false);
    };

    *handles.last_compacted_tail_id_arc.write().await = current_tail_id.clone();
    let compact_started_at_ms_handle = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compact_started_at_ms.clone())
    };
    if let Some(compact_started_at_ms_handle) = compact_started_at_ms_handle {
        *compact_started_at_ms_handle.write().await = Some(started_at_ms);
    }

    let compact_event = CompactRequest {
        session_id: session_id.to_string(),
        session_name: session_name.to_string(),
        messages: compact_msgs,
        from_id,
        to_id,
        parent_request: resolve_parent_request(active_sessions, session_id, parent_request).await,
        resume_completion_after_compact: false,
    };
    let log_from_id = compact_event.from_id.clone();
    let log_to_id = compact_event.to_id.clone();
    let app = app_handle.clone();
    let state_session_id = session_id.to_string();
    let state_session_name = session_name.to_string();
    tokio::spawn(async move {
        let state_event = crate::agent::llm::types::CompactStateEvent {
            session_id: state_session_id.clone(),
            session_name: Some(state_session_name),
            compacting: true,
            phase: crate::agent::llm::types::CompactStatePhase::Started,
            error: None,
        };
        if let Err(e) = app.emit("llm:compact-state", state_event) {
            log::error!("Failed to emit llm:compact-state: {}", e);
        }
        if let Err(e) = app.emit("llm:compact-request", compact_event) {
            log::error!("Failed to emit llm:compact-request: {}", e);
        }
    });
    log::info!(
        "🔧 Background compaction triggered: session={}, from_id={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
        session_id,
        log_from_id,
        log_to_id,
        split_idx,
        compacted_delta_count,
        reused_prior_summary,
        current_tail_id.as_deref().unwrap_or("?"),
        started_at_ms
    );

    Ok(true)
}

pub async fn trigger_post_response_compaction_if_needed(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    usage_total_tokens: usize,
    deferred_step: DeferredWorkflowStep,
) -> Result<bool, String> {
    let settings = load_context_management_settings().await;
    let safe_input_token_limit =
        std::cmp::min(settings.max_input_context, settings.model_max_limit);
    let should_trigger = uses_compaction_strategy(&settings.context_strategy)
        && usage_total_tokens > safe_input_token_limit;

    log::info!(
        "🧮 Post-response compaction evaluation: session={}, total_tokens={}, strategy={}, configured_max_input_context={}, model_max_limit={}, safe_input_token_limit={}, should_trigger={}",
        session_id,
        usage_total_tokens,
        settings.context_strategy,
        settings.max_input_context,
        settings.model_max_limit,
        safe_input_token_limit,
        should_trigger
    );

    if !should_trigger {
        return Ok(false);
    }

    let (handles, finalize_workflow_after_compact, deferred_workflow_step_handle) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            BackgroundCompactionHandles {
                compact_in_flight_arc: session.compact_in_flight.clone(),
                last_compacted_tail_id_arc: session.last_compacted_tail_id.clone(),
            },
            session.finalize_workflow_after_compact.clone(),
            session.deferred_workflow_step.clone(),
        )
    };

    finalize_workflow_after_compact.store(true, Ordering::SeqCst);
    *deferred_workflow_step_handle.write().await = Some(deferred_step);
    let triggered = trigger_background_compaction(
        active_sessions,
        app_handle,
        session_id,
        session_name,
        messages,
        None,
        &handles,
    )
    .await?;

    if !triggered {
        finalize_workflow_after_compact.store(false, Ordering::SeqCst);
        *deferred_workflow_step_handle.write().await = None;
    } else {
        log::info!(
            "🧹 Post-response compaction triggered synchronously from completed response usage: session={}, total_tokens={}, limit={}",
            session_id,
            usage_total_tokens,
            safe_input_token_limit
        );
    }

    Ok(triggered)
}

pub(crate) async fn try_trigger_preflight_compaction(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    parent_request: Option<CompactionParentRequest>,
) -> Result<bool, String> {
    if messages.len() <= 1 {
        return Ok(false);
    }

    let split_idx = find_preflight_compaction_split_index(messages);
    if split_idx == 0 {
        return Ok(false);
    }
    let (
        compact_in_flight_arc,
        last_compacted_tail_id_arc,
        awaiting_compact_arc,
        compact_context_handle,
    ) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.compact_in_flight.clone(),
            session.last_compacted_tail_id.clone(),
            session.awaiting_compact_completion.clone(),
            session.compact_context.clone(),
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
            error: None,
        };
        app_handle
            .emit("llm:compact-state", state_event)
            .map_err(|e| format!("Failed to emit llm:compact-state: {}", e))?;
        log::info!(
            "⏳ Reusing in-flight compaction and arming resume-after-compact: session={}, mode=preflight",
            session_id
        );
        return Ok(true);
    }

    let current_tail_id = messages.last().map(|m| m.id.clone());
    let last_compacted_tail = last_compacted_tail_id_arc.read().await.clone();
    if current_tail_id.as_deref() == last_compacted_tail.as_deref()
        && should_skip_same_tail_compaction(messages, split_idx)
    {
        compact_in_flight_arc.store(false, Ordering::SeqCst);
        return Ok(false);
    }

    let started_at_ms = chrono::Utc::now().timestamp_millis();
    let compact_context_record = compact_context_handle.read().await.clone();
    let Some((compact_msgs, from_id, to_id, compacted_delta_count, reused_prior_summary)) =
        build_compaction_request_payload(
            session_id,
            messages,
            split_idx,
            compact_context_record.as_ref(),
            started_at_ms,
        )
    else {
        compact_in_flight_arc.store(false, Ordering::SeqCst);
        return Ok(false);
    };

    awaiting_compact_arc.store(true, Ordering::SeqCst);
    *last_compacted_tail_id_arc.write().await = current_tail_id.clone();
    let compact_started_at_ms_handle = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compact_started_at_ms.clone())
    };
    if let Some(compact_started_at_ms_handle) = compact_started_at_ms_handle {
        *compact_started_at_ms_handle.write().await = Some(started_at_ms);
    }

    let compact_event = CompactRequest {
        session_id: session_id.to_string(),
        session_name: session_name.to_string(),
        messages: compact_msgs,
        from_id,
        to_id,
        parent_request: resolve_parent_request(active_sessions, session_id, parent_request).await,
        resume_completion_after_compact: true,
    };
    let log_from_id = compact_event.from_id.clone();
    let log_to_id = compact_event.to_id.clone();
    let state_event = crate::agent::llm::types::CompactStateEvent {
        session_id: session_id.to_string(),
        session_name: Some(session_name.to_string()),
        compacting: true,
        phase: crate::agent::llm::types::CompactStatePhase::Started,
        error: None,
    };

    app_handle
        .emit("llm:compact-state", state_event)
        .map_err(|e| format!("Failed to emit llm:compact-state: {}", e))?;
    app_handle
        .emit("llm:compact-request", compact_event)
        .map_err(|e| format!("Failed to emit llm:compact-request: {}", e))?;

    log::info!(
        "⏸️ Preflight compaction triggered: session={}, from_id={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
        session_id,
        log_from_id,
        log_to_id,
        split_idx,
        compacted_delta_count,
        reused_prior_summary,
        current_tail_id.as_deref().unwrap_or("?"),
        started_at_ms
    );

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

    let merged_messages = super::request::merge_consecutive_user_messages(messages);
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        session_id,
        &session_name,
        &merged_messages,
        None,
    )
    .await
}
