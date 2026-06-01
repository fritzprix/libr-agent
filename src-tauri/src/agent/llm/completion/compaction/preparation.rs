use crate::agent::llm::load_context_management_settings;
use crate::agent::llm::types::{CompactRequest, CompactionParentRequest};
use crate::agent::state::{AgentSession, CompactionRecoveryPhase};
use crate::mcp::types::MCPTool;
use crate::models::chat::Message;
use crate::repositories::CompactContextRecord;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::diagnostics::log_compaction_input_diagnostics;
use super::payload::{
    apply_compaction_retry_budget, build_compaction_request_payload,
    build_overflow_recovery_compaction_messages, estimate_compaction_non_message_tokens,
    fit_compaction_request_messages_to_limit, CompactionRequestPayload,
};

pub(super) const MAX_COMPACTION_BUDGET_RETRY_ATTEMPTS: u32 = 3;
pub(super) const MAX_COMPACTION_SPLIT_BACKOFF_ATTEMPTS: usize = 3;

pub(super) struct PreparedCompactionRequest {
    pub compact_event: CompactRequest,
    pub started_at_ms: i64,
    pub current_tail_id: Option<String>,
    pub compacted_delta_count: usize,
    pub reused_prior_summary: bool,
}

pub(super) struct PreparedCompactionAttempt {
    pub prepared: PreparedCompactionRequest,
    pub recovery_phase: CompactionRecoveryPhase,
    pub retry_attempt: u32,
}

#[derive(Clone)]
pub(super) struct PrepareCompactionRequestInput<'a> {
    pub session_id: &'a str,
    pub session_name: &'a str,
    pub messages: &'a [Message],
    pub split_idx: usize,
    pub measured_output_tokens_reserve: usize,
    pub parent_request: Option<CompactionParentRequest>,
    pub compact_context_record: Option<CompactContextRecord>,
    pub started_at_ms: i64,
    pub resume_completion_after_compact: bool,
    pub recovery_phase: CompactionRecoveryPhase,
    pub retry_attempt: u32,
}

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

async fn prepare_compaction_request(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    input: PrepareCompactionRequestInput<'_>,
) -> Result<Option<PreparedCompactionRequest>, String> {
    let PrepareCompactionRequestInput {
        session_id,
        session_name,
        messages,
        split_idx,
        measured_output_tokens_reserve,
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
        std::cmp::min(settings.max_input_context(), settings.model_max_limit);
    let base_effective_input_budget =
        crate::agent::llm::token_utils::calculate_effective_input_budget(
            safe_input_token_limit,
            measured_output_tokens_reserve,
        );
    let effective_input_token_limit =
        apply_compaction_retry_budget(base_effective_input_budget, retry_attempt);
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
            "🔧 Applying compaction retry budget: session={}, retry_attempt={}, safe_input_token_limit={}, measured_output_tokens_reserve={}, base_effective_input_budget={}, effective_input_token_limit={}",
            session_id,
            retry_attempt,
            safe_input_token_limit,
            measured_output_tokens_reserve,
            base_effective_input_budget,
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

pub(super) async fn prepare_compaction_request_with_recovery_ladder(
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
