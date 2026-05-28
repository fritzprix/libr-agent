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
    fit_compaction_request_messages_to_limit, inspect_compaction_payload,
    CompactionPayloadDiagnostics, CompactionRequestPayload,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailRecompactionRecoveryPlan {
    pub compacted_to_idx: usize,
    pub first_delta_message_idx: usize,
    pub latest_request_start_idx: usize,
    pub fallback_split_idx: usize,
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

fn format_compaction_payload_messages(diagnostics: &CompactionPayloadDiagnostics) -> String {
    if diagnostics.messages.is_empty() {
        return "  - <none>".to_string();
    }

    diagnostics
        .messages
        .iter()
        .map(|message| {
            format!(
                "  - {} | role={} | source={} | flags={} | preview={}",
                message.id,
                message.role,
                message.source,
                if message.flags.is_empty() {
                    "-".to_string()
                } else {
                    message.flags.join("+")
                },
                message.preview
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn log_compaction_input_diagnostics(
    session_id: &str,
    provider_id: &str,
    recovery_phase: CompactionRecoveryPhase,
    retry_attempt: u32,
    base_payload_message_count: usize,
    request_layout_message_count: usize,
    selected_messages: &[Message],
) {
    let diagnostics = inspect_compaction_payload(selected_messages);

    log::info!(
        "🧪 Compaction input diagnostics: session={}, provider={}, recovery_phase={:?}, retry_attempt={}, base_payload_message_count={}, request_layout_message_count={}, emitted_message_count={}, body_message_count={}, raw_delta_message_count={}, compact_summary_count={}, compaction_instruction_count={}, scaffolding_count={}, external_request_count={}, assistant_message_count={}, tool_message_count={}, latest_external_request_ids={:?}",
        session_id,
        provider_id,
        recovery_phase,
        retry_attempt,
        base_payload_message_count,
        request_layout_message_count,
        diagnostics.total_messages,
        diagnostics.body_message_count,
        diagnostics.raw_delta_message_count,
        diagnostics.compact_summary_count,
        diagnostics.compaction_instruction_count,
        diagnostics.scaffolding_count,
        diagnostics.external_request_count,
        diagnostics.assistant_message_count,
        diagnostics.tool_message_count,
        diagnostics.latest_external_request_message_ids
    );

    if diagnostics.external_request_count == 0 {
        log::warn!(
            "⚠️ Compaction input emitted without any external request messages: session={}, provider={}, recovery_phase={:?}, retry_attempt={}",
            session_id,
            provider_id,
            recovery_phase,
            retry_attempt
        );
    }

    if diagnostics.raw_delta_message_count == 1 && diagnostics.external_request_count == 1 {
        log::warn!(
            "⚠️ Compaction input collapsed to a single raw delta message around the latest request: session={}, provider={}, recovery_phase={:?}, retry_attempt={}",
            session_id,
            provider_id,
            recovery_phase,
            retry_attempt
        );
    }

    log::debug!(
        "🧾 Compaction input messages: session={}, provider={}, recovery_phase={:?}, retry_attempt={}, messages=\n{}",
        session_id,
        provider_id,
        recovery_phase,
        retry_attempt,
        format_compaction_payload_messages(&diagnostics)
    );
}

pub fn preview_preflight_compaction_selection(messages: &[Message]) -> CompactionSelectionPreview {
    build_compaction_selection_preview(
        messages,
        find_preflight_compactable_end_exclusive(messages, None, None),
    )
}

fn derive_tail_recompaction_recovery_plan(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    compactable_end_exclusive: usize,
) -> Option<TailRecompactionRecoveryPlan> {
    let record = compact_context_record?;
    let compacted_to_idx = messages
        .iter()
        .position(|message| message.id == record.to_id)?;
    let first_delta_message_idx = compacted_to_idx.saturating_add(1);
    if first_delta_message_idx < compactable_end_exclusive {
        return None;
    }

    let latest_request_start_idx = super::find_latest_external_request_seed_block_start(messages)?;
    if latest_request_start_idx <= first_delta_message_idx {
        return None;
    }

    Some(TailRecompactionRecoveryPlan {
        compacted_to_idx,
        first_delta_message_idx,
        latest_request_start_idx,
        fallback_split_idx: latest_request_start_idx,
    })
}

#[allow(dead_code)]
pub fn derive_tail_recompaction_recovery_plan_for_testing(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    compactable_end_exclusive: usize,
) -> Option<TailRecompactionRecoveryPlan> {
    derive_tail_recompaction_recovery_plan(
        messages,
        compact_context_record,
        compactable_end_exclusive,
    )
}

fn find_compaction_delta_start_index(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
) -> usize {
    compact_context_record
        .and_then(|record| {
            messages
                .iter()
                .position(|message| message.id == record.to_id)
                .map(|idx| idx.saturating_add(1))
        })
        .unwrap_or(0)
}

fn find_latest_checkpoint_index(messages: &[Message]) -> Option<usize> {
    messages
        .iter()
        .rposition(|message| message.prompt_tokens_value().is_some())
}

fn find_prompt_checkpoint_compactable_end_exclusive(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    current_context_limit: usize,
) -> Option<usize> {
    let delta_start_idx = find_compaction_delta_start_index(messages, compact_context_record);
    let uncompacted_messages = &messages[delta_start_idx..];
    let latest_checkpoint_relative_idx = find_latest_checkpoint_index(uncompacted_messages)?;
    let latest_checkpoint = &uncompacted_messages[latest_checkpoint_relative_idx];
    let latest_prompt_tokens = latest_checkpoint.prompt_tokens_value()?;

    if latest_prompt_tokens > current_context_limit {
        let compaction_window_start = latest_prompt_tokens.saturating_sub(current_context_limit);
        let preserve_relative_idx = uncompacted_messages
            .iter()
            .position(|message| {
                message
                    .prompt_tokens_value()
                    .is_some_and(|value| value > compaction_window_start)
            })
            .unwrap_or(latest_checkpoint_relative_idx);
        return Some(delta_start_idx + preserve_relative_idx);
    }

    Some(delta_start_idx + latest_checkpoint_relative_idx + 1)
}

pub fn has_prompt_checkpoint_compaction_target(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    current_context_limit: usize,
) -> bool {
    find_prompt_checkpoint_compactable_end_exclusive(
        messages,
        compact_context_record,
        current_context_limit,
    )
    .is_some_and(|compactable_end_exclusive| {
        compactable_end_exclusive
            > find_compaction_delta_start_index(messages, compact_context_record)
    })
}

fn find_preflight_compactable_end_exclusive(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    current_context_limit: Option<usize>,
) -> usize {
    if messages.is_empty() {
        return 0;
    }

    if let Some(limit) = current_context_limit {
        if let Some(prompt_checkpoint_end_exclusive) =
            find_prompt_checkpoint_compactable_end_exclusive(
                messages,
                compact_context_record,
                limit,
            )
        {
            return prompt_checkpoint_end_exclusive;
        }
    }

    let delta_start_idx = find_compaction_delta_start_index(messages, compact_context_record);
    let relative_end_exclusive = crate::agent::llm::context_selector::find_compaction_split_index(
        &messages[delta_start_idx..],
    );
    delta_start_idx + relative_end_exclusive
}

pub fn find_preflight_compactable_end_exclusive_for_testing(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    current_context_limit: Option<usize>,
) -> usize {
    find_preflight_compactable_end_exclusive(
        messages,
        compact_context_record,
        current_context_limit,
    )
}

fn log_preflight_split_boundary(
    session_id: &str,
    messages: &[Message],
    split_idx: usize,
    reason: &str,
) {
    let diagnostics =
        crate::agent::llm::context_selector::inspect_compaction_split_boundary(messages);
    let first_message_id = messages
        .first()
        .map(|message| message.id.as_str())
        .unwrap_or("?");
    let last_message_id = messages
        .last()
        .map(|message| message.id.as_str())
        .unwrap_or("?");
    let first_message_role = messages
        .first()
        .map(|message| message.role.as_str())
        .unwrap_or("?");
    let last_message_role = messages
        .last()
        .map(|message| message.role.as_str())
        .unwrap_or("?");

    log::warn!(
        "🧭 Preflight compaction split diagnostics: session={}, reason={}, message_count={}, split_idx={}, first_unresolved_owner_index={:?}, first_unresolved_owner_id={:?}, first_unresolved_tool_call_count={}, first_message_id={}, first_message_role={}, last_message_id={}, last_message_role={}",
        session_id,
        reason,
        messages.len(),
        split_idx,
        diagnostics.first_unresolved_owner_index,
        diagnostics.first_unresolved_owner_id,
        diagnostics.first_unresolved_tool_call_count,
        first_message_id,
        first_message_role,
        last_message_id,
        last_message_role
    );
}

pub fn should_skip_same_tail_compaction(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    compactable_end_exclusive: usize,
) -> bool {
    let has_compact_summary = messages
        .first()
        .map(|message| message.is_compact_summary())
        .unwrap_or(false);

    if !has_compact_summary {
        return false;
    }

    compactable_end_exclusive <= find_compaction_delta_start_index(messages, compact_context_record)
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
        compact_messages: base_compact_messages,
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
        let compact_record_summary = compact_context_record.as_ref().map(|record| {
            let compacted_to_idx = messages
                .iter()
                .position(|message| message.id == record.to_id);
            let first_delta_message_idx = compacted_to_idx.map(|idx| idx.saturating_add(1));
            format!(
                "to_id={}, compacted_to_idx={:?}, first_delta_message_idx={:?}",
                record.to_id, compacted_to_idx, first_delta_message_idx
            )
        });
        log::warn!(
            "⏭️ Preflight compaction payload build returned no-op: session={}, split_idx={}, message_count={}, compact_record={:?}",
            session_id,
            split_idx,
            messages.len(),
            compact_record_summary
        );
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
            base_compact_messages.clone(),
        )
    });
    let final_compact_messages = request_layout
        .as_ref()
        .map(|layout| layout.messages.clone())
        .unwrap_or_else(|| base_compact_messages.clone());
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

    log_compaction_input_diagnostics(
        session_id,
        provider_id,
        recovery_phase,
        retry_attempt,
        base_compact_messages.len(),
        final_compact_messages.len(),
        &compact_messages,
    );

    let mut final_compacted_delta_count = compacted_delta_count;

    if !compact_messages.is_empty() {
        // Count how many delta messages are actually in compact_messages
        // (excluding the compaction instruction/overlay at the end).
        let delta_only_count = compact_messages
            .iter()
            .filter(|m| {
                !m.is_compaction_overlay_message()
                    && !m.is_compact_summary()
                    && !m.is_request_layout_scaffolding_message()
            })
            .count();
        final_compacted_delta_count = delta_only_count;
    }

    if final_compacted_delta_count != compacted_delta_count {
        log::info!(
            "📐 Compaction payload shrunken, adjusting delta metadata: session={}, compacted_delta_count: {} -> {}",
            session_id,
            compacted_delta_count,
            final_compacted_delta_count
        );
    }

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
            to_id,
            compacted_delta_count: final_compacted_delta_count,
            parent_request: final_parent_request,
            resume_completion_after_compact,
        },
        started_at_ms,
        current_tail_id: messages.last().map(|message| message.id.clone()),
        compacted_delta_count: final_compacted_delta_count,
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
        log::info!(
            "⏭️ Preflight compaction skipped (insufficient messages): session={}, count={}",
            session_id,
            messages.len()
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
    let compact_context_record = compact_context_handle.read().await.clone();
    let settings = load_context_management_settings().await;
    let current_context_limit = std::cmp::min(settings.max_input_context, settings.model_max_limit);
    let compactable_end_exclusive = find_preflight_compactable_end_exclusive(
        messages,
        compact_context_record.as_ref(),
        Some(current_context_limit),
    );
    if compactable_end_exclusive == 0 {
        log_preflight_split_boundary(
            session_id,
            messages,
            compactable_end_exclusive,
            "split_idx_zero",
        );
        log::warn!(
            "⏭️ Preflight compaction skipped (split_idx=0): session={}",
            session_id
        );
        return Ok(false);
    }

    let current_tail_id = messages.last().map(|message| message.id.clone());
    let last_compacted_tail = compaction.last_compacted_tail_id().await;
    if current_tail_id.as_deref() == last_compacted_tail.as_deref()
        && should_skip_same_tail_compaction(
            messages,
            compact_context_record.as_ref(),
            compactable_end_exclusive,
        )
    {
        log::info!(
            "⏭️ Preflight compaction skipped (same tail): session={}, tail={}, split_idx={}, message_count={}, last_compacted_tail={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?"),
            compactable_end_exclusive,
            messages.len(),
            last_compacted_tail.as_deref().unwrap_or("?")
        );
        return Ok(false);
    }

    let started_at_ms = chrono::Utc::now().timestamp_millis();
    let initial_retry_attempt = compaction.retry_attempt().await;
    let initial_recovery_phase = compaction.recovery_phase().await;
    let mut prepared_attempt = prepare_compaction_request_with_recovery_ladder(
        active_sessions,
        PrepareCompactionRequestInput {
            session_id,
            session_name,
            messages,
            split_idx: compactable_end_exclusive,
            parent_request: parent_request.clone(),
            compact_context_record: compact_context_record.clone(),
            started_at_ms,
            resume_completion_after_compact,
            recovery_phase: initial_recovery_phase,
            retry_attempt: initial_retry_attempt,
        },
        initial_recovery_phase,
        initial_retry_attempt,
    )
    .await?;

    if prepared_attempt.is_none() {
        if let Some(recovery_plan) = derive_tail_recompaction_recovery_plan(
            messages,
            compact_context_record.as_ref(),
            compactable_end_exclusive,
        ) {
            log::warn!(
                "🧯 Forcing tail re-compaction recovery after no-op incremental payload: session={}, original_split_idx={}, fallback_split_idx={}, compacted_to_idx={}, first_delta_message_idx={}, latest_request_start_idx={}, message_count={}",
                session_id,
                compactable_end_exclusive,
                recovery_plan.fallback_split_idx,
                recovery_plan.compacted_to_idx,
                recovery_plan.first_delta_message_idx,
                recovery_plan.latest_request_start_idx,
                messages.len()
            );
            prepared_attempt = prepare_compaction_request_with_recovery_ladder(
                active_sessions,
                PrepareCompactionRequestInput {
                    session_id,
                    session_name,
                    messages,
                    split_idx: recovery_plan.fallback_split_idx,
                    parent_request: parent_request.clone(),
                    compact_context_record: compact_context_record.clone(),
                    started_at_ms,
                    resume_completion_after_compact,
                    recovery_phase: crate::agent::state::CompactionRecoveryPhase::OverflowRecovery,
                    retry_attempt: 0,
                },
                crate::agent::state::CompactionRecoveryPhase::OverflowRecovery,
                0,
            )
            .await?;
        }
    }

    let Some(prepared_attempt) = prepared_attempt else {
        log::info!(
            "⏭️ Preflight compaction skipped (no new delta beyond previous summary): session={}, tail={}, split_idx={}, message_count={}, last_compacted_tail={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?"),
            compactable_end_exclusive,
            messages.len(),
            last_compacted_tail.as_deref().unwrap_or("?")
        );
        return Ok(false);
    };
    let PreparedCompactionAttempt {
        prepared,
        recovery_phase,
        retry_attempt,
    } = prepared_attempt;

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
            "⏸️ Preflight compaction triggered: session={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
            session_id,
            log_to_id,
            compactable_end_exclusive,
            prepared.compacted_delta_count,
            prepared.reused_prior_summary,
            current_tail_id.as_deref().unwrap_or("?"),
            prepared.started_at_ms
        );
    } else {
        log::info!(
            "🧰 Manual compaction triggered: session={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
            session_id,
            log_to_id,
            compactable_end_exclusive,
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
