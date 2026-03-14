use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use super::context::{load_context_management_settings, uses_compaction_strategy};
use crate::agent::llm::types::CompactRequest;

pub fn should_trigger_background_compaction(
    current_tokens: usize,
    safe_input_token_limit: usize,
    context_strategy: &str,
) -> bool {
    uses_compaction_strategy(context_strategy)
        && current_tokens
            > crate::agent::llm::token_utils::calculate_compact_threshold(safe_input_token_limit)
}

pub(crate) async fn trigger_background_compaction(
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

pub(crate) async fn try_trigger_preflight_compaction(
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

    let merged_messages = super::request::merge_consecutive_user_messages(messages);
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        session_id,
        &session_name,
        &merged_messages,
    )
    .await
}
