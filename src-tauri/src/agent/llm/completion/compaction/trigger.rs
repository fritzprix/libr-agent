use crate::agent::llm::completion::request::normalize_request_messages;
use crate::agent::llm::load_context_management_settings;
use crate::agent::llm::types::{CompactRequest, CompactionParentRequest};
use crate::agent::state::{
    AgentSession, CompactionBeginOutcome, CompactionKind, CompactionRecoveryPhase,
    CompactionReuseOutcome,
};
use crate::agent::tauri_events::{emit_compact_request, emit_compact_started};
use crate::mcp::types::MCPTool;
use crate::models::chat::Message;
use crate::repositories::CompactContextRecord;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::payload::{
    apply_compaction_retry_budget, build_compaction_request_payload,
    build_overflow_recovery_compaction_messages, estimate_compaction_non_message_tokens,
    fit_compaction_request_messages_to_limit, CompactionRequestPayload,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionSelectionPreview {
    pub compacted_ids: Vec<String>,
    pub preserved_ids: Vec<String>,
}

fn build_compaction_selection_preview(
    messages: &[Message],
    split_idx: usize,
) -> CompactionSelectionPreview {
    let split_idx = split_idx.min(messages.len());

    CompactionSelectionPreview {
        compacted_ids: messages[..split_idx]
            .iter()
            .map(|message| message.id.clone())
            .collect(),
        preserved_ids: messages[split_idx..]
            .iter()
            .map(|message| message.id.clone())
            .collect(),
    }
}

pub fn preview_preflight_compaction_selection(messages: &[Message]) -> CompactionSelectionPreview {
    build_compaction_selection_preview(messages, find_preflight_compaction_split_index(messages))
}

fn find_preflight_compaction_split_index(messages: &[Message]) -> usize {
    if messages.is_empty() {
        return 0;
    }

    crate::agent::llm::context_selector::find_compaction_split_index(messages)
}

pub fn should_skip_same_tail_compaction(messages: &[Message], split_idx: usize) -> bool {
    let has_compact_summary = messages
        .first()
        .map(|message| message.is_compact_summary())
        .unwrap_or(false);

    if !has_compact_summary {
        return false;
    }

    split_idx <= 1
}

fn filter_compaction_history_messages(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .filter(|message| {
            !message.is_request_layout_scaffolding_message()
                && !message.is_compaction_overlay_message()
        })
        .cloned()
        .collect()
}

async fn load_merged_compaction_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(String, Vec<Message>), String> {
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
            .cloned()
            .collect::<Vec<_>>();

        (session_name, messages)
    };

    Ok((
        session_name,
        normalize_request_messages(filter_compaction_history_messages(&messages)),
    ))
}

struct PreparedCompactionRequest {
    compact_event: CompactRequest,
    started_at_ms: i64,
    current_tail_id: Option<String>,
    compacted_delta_count: usize,
    reused_prior_summary: bool,
}

struct PreparedCompactionAttempt {
    prepared: PreparedCompactionRequest,
    recovery_phase: CompactionRecoveryPhase,
    retry_attempt: u32,
}

const MAX_COMPACTION_BUDGET_RETRY_ATTEMPTS: u32 = 3;

fn advance_compaction_overflow_recovery_step(
    recovery_phase: CompactionRecoveryPhase,
    retry_attempt: u32,
) -> Option<(CompactionRecoveryPhase, u32)> {
    match recovery_phase {
        CompactionRecoveryPhase::CacheAligned
            if retry_attempt < MAX_COMPACTION_BUDGET_RETRY_ATTEMPTS =>
        {
            Some((CompactionRecoveryPhase::CacheAligned, retry_attempt + 1))
        }
        CompactionRecoveryPhase::CacheAligned => {
            Some((CompactionRecoveryPhase::OverflowRecovery, 0))
        }
        CompactionRecoveryPhase::OverflowRecovery => {
            Some((CompactionRecoveryPhase::DegradedTools, 0))
        }
        CompactionRecoveryPhase::DegradedTools => None,
    }
}

pub fn advance_compaction_overflow_recovery_step_for_testing(
    recovery_phase: CompactionRecoveryPhase,
    retry_attempt: u32,
) -> Option<(CompactionRecoveryPhase, u32)> {
    advance_compaction_overflow_recovery_step(recovery_phase, retry_attempt)
}

#[derive(Clone)]
struct PrepareCompactionRequestInput<'a> {
    session_id: &'a str,
    session_name: &'a str,
    messages: &'a [Message],
    split_idx: usize,
    parent_request: Option<CompactionParentRequest>,
    compact_context_record: Option<CompactContextRecord>,
    started_at_ms: i64,
    resume_completion_after_compact: bool,
    recovery_phase: CompactionRecoveryPhase,
    retry_attempt: u32,
}

fn degrade_tools_for_overflow_recovery(tools: &[MCPTool]) -> Vec<MCPTool> {
    tools
        .iter()
        .map(|tool| MCPTool {
            name: tool.name.clone(),
            title: tool.title.clone(),
            description: tool.description.clone(),
            // Overflow recovery is the last-resort fit path. We intentionally keep
            // only high-signal tool identity/description and drop verbose schemas so
            // the model still sees what tools exist while the payload stays small
            // enough to survive the degraded-tools phase.
            input_schema: crate::mcp::schema::MCPToolInputSchema::default(),
            output_schema: None,
            annotations: tool.annotations.clone(),
        })
        .collect()
}

async fn prepare_compaction_request(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    input: PrepareCompactionRequestInput<'_>,
) -> Result<Option<PreparedCompactionRequest>, String> {
    let PrepareCompactionRequestInput {
        session_id,
        session_name,
        messages,
        split_idx,
        parent_request,
        compact_context_record,
        started_at_ms,
        resume_completion_after_compact,
        recovery_phase,
        retry_attempt,
    } = input;

    let Some(CompactionRequestPayload {
        compact_messages,
        from_id,
        to_id,
        compacted_delta_count,
        reused_prior_summary,
    }) = build_compaction_request_payload(
        session_id,
        messages,
        split_idx,
        compact_context_record.as_ref(),
        started_at_ms,
    )
    else {
        return Ok(None);
    };

    let settings = load_context_management_settings().await;
    let safe_input_token_limit =
        std::cmp::min(settings.max_input_context, settings.model_max_limit);
    let effective_input_token_limit =
        apply_compaction_retry_budget(safe_input_token_limit, retry_attempt);
    let resolved_parent_request =
        resolve_parent_request(active_sessions, session_id, parent_request).await;
    let request_layout = resolved_parent_request.as_ref().map(|request| {
        crate::agent::llm::build_request_layout(
            &request.provider,
            session_id,
            request.system_prompt.clone(),
            request.session_context.clone(),
            compact_messages.clone(),
        )
    });
    let final_compact_messages = request_layout
        .as_ref()
        .map(|layout| layout.messages.clone())
        .unwrap_or_else(|| compact_messages.clone());
    let final_parent_request = match (resolved_parent_request, request_layout.as_ref()) {
        (Some(mut request), Some(layout)) => {
            request.system_prompt = layout.system_prompt.clone();
            Some(request)
        }
        (None, Some(_)) => None,
        (request, None) => request,
    };
    let final_parent_request = match (recovery_phase, final_parent_request) {
        (CompactionRecoveryPhase::DegradedTools, Some(mut request)) => {
            request.available_tools = request
                .available_tools
                .as_ref()
                .map(|tools| degrade_tools_for_overflow_recovery(tools));
            Some(request)
        }
        (_, request) => request,
    };
    let (system_prompt_tokens, tools_tokens) =
        estimate_compaction_non_message_tokens(final_parent_request.as_ref());
    let provider_id = final_parent_request
        .as_ref()
        .map(|request| request.provider.as_str())
        .unwrap_or("openai");
    let compact_messages = match recovery_phase {
        CompactionRecoveryPhase::CacheAligned => fit_compaction_request_messages_to_limit(
            &final_compact_messages,
            provider_id,
            effective_input_token_limit,
            system_prompt_tokens,
            tools_tokens,
        )?,
        CompactionRecoveryPhase::OverflowRecovery | CompactionRecoveryPhase::DegradedTools => {
            build_overflow_recovery_compaction_messages(
                &final_compact_messages,
                provider_id,
                effective_input_token_limit,
                system_prompt_tokens,
                tools_tokens,
            )?
        }
    };

    if retry_attempt > 0 {
        log::warn!(
            "🔧 Applying compaction retry budget: session={}, retry_attempt={}, safe_input_token_limit={}, effective_input_token_limit={}",
            session_id,
            retry_attempt,
            safe_input_token_limit,
            effective_input_token_limit
        );
    }
    if !matches!(recovery_phase, CompactionRecoveryPhase::CacheAligned) {
        log::warn!(
            "🩹 Applying compaction overflow recovery: session={}, recovery_phase={:?}, tools_degraded={}",
            session_id,
            recovery_phase,
            matches!(recovery_phase, CompactionRecoveryPhase::DegradedTools)
        );
    }

    Ok(Some(PreparedCompactionRequest {
        compact_event: CompactRequest {
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            messages: compact_messages,
            from_id,
            to_id,
            parent_request: final_parent_request,
            resume_completion_after_compact,
        },
        started_at_ms,
        current_tail_id: messages.last().map(|message| message.id.clone()),
        compacted_delta_count,
        reused_prior_summary,
    }))
}

async fn prepare_compaction_request_with_recovery_ladder(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    input: PrepareCompactionRequestInput<'_>,
    initial_recovery_phase: CompactionRecoveryPhase,
    initial_retry_attempt: u32,
) -> Result<Option<PreparedCompactionAttempt>, String> {
    let mut recovery_phase = initial_recovery_phase;
    let mut retry_attempt = initial_retry_attempt;

    loop {
        match prepare_compaction_request(
            active_sessions,
            PrepareCompactionRequestInput {
                recovery_phase,
                retry_attempt,
                ..input.clone()
            },
        )
        .await
        {
            Ok(Some(prepared)) => {
                return Ok(Some(PreparedCompactionAttempt {
                    prepared,
                    recovery_phase,
                    retry_attempt,
                }));
            }
            Ok(None) => return Ok(None),
            Err(error) => {
                let Some((next_recovery_phase, next_retry_attempt)) =
                    advance_compaction_overflow_recovery_step(recovery_phase, retry_attempt)
                else {
                    return Err(error);
                };
                log::warn!(
                    "🪜 Advancing local compaction overflow recovery before emit: session={}, from_phase={:?}, from_retry_attempt={}, to_phase={:?}, to_retry_attempt={}",
                    input.session_id,
                    recovery_phase,
                    retry_attempt,
                    next_recovery_phase,
                    next_retry_attempt
                );
                recovery_phase = next_recovery_phase;
                retry_attempt = next_retry_attempt;
            }
        }
    }
}

pub(crate) async fn try_trigger_preflight_compaction(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    parent_request: Option<CompactionParentRequest>,
    resume_completion_after_compact: bool,
) -> Result<bool, String> {
    if messages.len() <= 1 {
        log::debug!(
            "⏭️ Preflight compaction skipped (insufficient messages): session={}, count={}",
            session_id,
            messages.len()
        );
        return Ok(false);
    }

    let split_idx = find_preflight_compaction_split_index(messages);
    if split_idx == 0 {
        log::debug!(
            "⏭️ Preflight compaction skipped (split_idx=0): session={}",
            session_id
        );
        return Ok(false);
    }

    let (compact_context_handle, compaction) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (session.compact_context.clone(), session.compaction.clone())
    };

    let current_tail_id = messages.last().map(|message| message.id.clone());
    let last_compacted_tail = compaction.last_compacted_tail_id().await;
    if current_tail_id.as_deref() == last_compacted_tail.as_deref()
        && should_skip_same_tail_compaction(messages, split_idx)
    {
        log::debug!(
            "⏭️ Preflight compaction skipped (same tail): session={}, tail={}, split_idx={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?"),
            split_idx
        );
        return Ok(false);
    }

    let started_at_ms = chrono::Utc::now().timestamp_millis();
    let compact_context_record = compact_context_handle.read().await.clone();
    let initial_retry_attempt = compaction.retry_attempt().await;
    let initial_recovery_phase = compaction.recovery_phase().await;
    let Some(prepared_attempt) = prepare_compaction_request_with_recovery_ladder(
        active_sessions,
        PrepareCompactionRequestInput {
            session_id,
            session_name,
            messages,
            split_idx,
            parent_request,
            compact_context_record,
            started_at_ms,
            resume_completion_after_compact,
            recovery_phase: initial_recovery_phase,
            retry_attempt: initial_retry_attempt,
        },
        initial_recovery_phase,
        initial_retry_attempt,
    )
    .await?
    else {
        log::debug!(
            "⏭️ Preflight compaction skipped (no new delta beyond previous summary): session={}, tail={}, split_idx={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?"),
            split_idx
        );
        return Ok(false);
    };
    let PreparedCompactionAttempt {
        prepared,
        recovery_phase,
        retry_attempt,
    } = prepared_attempt;

    let log_from_id = prepared.compact_event.from_id.clone();
    let log_to_id = prepared.compact_event.to_id.clone();
    let compact_event = prepared.compact_event.clone();
    let kind = if resume_completion_after_compact {
        CompactionKind::Preflight
    } else {
        CompactionKind::Manual
    };
    match compaction
        .try_begin(
            kind,
            prepared.current_tail_id.clone(),
            prepared.started_at_ms,
        )
        .await
    {
        CompactionBeginOutcome::Started => {
            compaction
                .set_recovery_progress(recovery_phase, retry_attempt)
                .await;
            if let Err(error) = emit_compact_started(
                app_handle,
                session_id.to_string(),
                Some(session_name.to_string()),
                resume_completion_after_compact,
            ) {
                compaction.clear_runtime_state(true).await;
                compaction.reset_recovery_progress().await;
                return Err(error);
            }
            if let Err(error) = emit_compact_request(app_handle, compact_event) {
                compaction.clear_runtime_state(true).await;
                compaction.reset_recovery_progress().await;
                return Err(error);
            }
        }
        CompactionBeginOutcome::AlreadyInFlight => {
            let reuse_outcome = if resume_completion_after_compact {
                compaction.arm_resume_completion().await
            } else {
                CompactionReuseOutcome::NoChange
            };
            emit_compact_started(
                app_handle,
                session_id.to_string(),
                Some(session_name.to_string()),
                resume_completion_after_compact,
            )?;
            if resume_completion_after_compact {
                match reuse_outcome {
                    CompactionReuseOutcome::Promoted | CompactionReuseOutcome::NoChange => {
                        log::info!(
                            "⏳ Reusing in-flight compaction and arming resume-after-compact: session={}, mode=preflight",
                            session_id
                        );
                    }
                    CompactionReuseOutcome::NotInFlight => {
                        return Err(format!(
                            "Compaction phase unexpectedly became idle while reusing preflight compaction for session {}",
                            session_id
                        ));
                    }
                }
            } else {
                log::info!(
                    "⏳ Reusing in-flight compaction without resume-after-compact: session={}, mode=manual",
                    session_id
                );
            }
            return Ok(true);
        }
    }

    if resume_completion_after_compact {
        log::info!(
            "⏸️ Preflight compaction triggered: session={}, from_id={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
            session_id,
            log_from_id,
            log_to_id,
            split_idx,
            prepared.compacted_delta_count,
            prepared.reused_prior_summary,
            current_tail_id.as_deref().unwrap_or("?"),
            prepared.started_at_ms
        );
    } else {
        log::info!(
            "🧰 Manual compaction triggered: session={}, from_id={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
            session_id,
            log_from_id,
            log_to_id,
            split_idx,
            prepared.compacted_delta_count,
            prepared.reused_prior_summary,
            current_tail_id.as_deref().unwrap_or("?"),
            prepared.started_at_ms
        );
    }

    Ok(true)
}

pub async fn trigger_preflight_compaction_for_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<bool, String> {
    let (session_name, merged_messages) =
        load_merged_compaction_messages(active_sessions, session_id).await?;
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        session_id,
        &session_name,
        &merged_messages,
        None,
        true,
    )
    .await
}

pub async fn trigger_manual_compaction_for_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<bool, String> {
    let (session_name, merged_messages) =
        load_merged_compaction_messages(active_sessions, session_id).await?;
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        session_id,
        &session_name,
        &merged_messages,
        None,
        false,
    )
    .await
}
