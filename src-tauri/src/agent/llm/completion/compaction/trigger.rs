//! Preflight compaction trigger.
//!
//! Contract (see `docs/specs/message-compaction.md` §5.2):
//!
//! 1. Select a **resume-fit** split first (`select_resume_fit_compaction_split`).
//! 2. Iterate candidates deep → shallow; **skip** any split whose projected
//!    post-compact resume still exceeds `effective_input_budget`.
//! 3. Accept prepare() only for a resume-fit candidate — never commit a shallow
//!    checkpoint-seeded split merely because the compaction LLM input fitted.
//! 4. Log `checkpoint_seed_split_idx` separately from `preferred_split_idx` /
//!    `chosen_split_idx` so regressions are visible in production logs.

use crate::agent::llm::completion::request::normalize_request_messages;
use crate::agent::llm::load_context_management_settings;
use crate::agent::state::{
    AgentSession, CompactionBeginOutcome, CompactionKind, CompactionReuseOutcome,
};
use crate::agent::tauri_events::{emit_compact_request, emit_compact_started};
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::diagnostics::log_preflight_split_boundary;
use super::preparation::{
    prepare_compaction_request_with_recovery_ladder, PrepareCompactionRequestInput,
    PreparedCompactionAttempt, MAX_COMPACTION_SPLIT_BACKOFF_ATTEMPTS,
};
use super::selection::{
    build_resume_fit_split_candidates, derive_tail_recompaction_recovery_plan,
    estimate_post_compact_resume_tokens, select_resume_fit_compaction_split,
    COMPACT_SUMMARY_PLACEHOLDER_TOKENS,
};

// Re-export public items to keep the external API and downstream imports stable.
pub use super::preparation::advance_compaction_overflow_recovery_step_for_testing;
pub use super::selection::{
    build_checkpoint_backoff_split_candidates_for_testing,
    derive_tail_recompaction_recovery_plan_for_testing,
    estimate_post_compact_resume_tokens_for_testing,
    find_preflight_compactable_end_exclusive_for_testing, find_resume_fit_split_idx_for_testing,
    has_prompt_checkpoint_compaction_target, preview_preflight_compaction_selection,
    select_resume_fit_compaction_split_for_testing, should_skip_same_tail_compaction,
    CompactionSelectionPreview, ResumeFitSplitSelection, TailRecompactionRecoveryPlan,
};

#[derive(Clone)]
pub(crate) struct PreflightCompactionTriggerInput<'a> {
    pub session_id: &'a str,
    pub session_name: &'a str,
    pub messages: &'a [Message],
    pub parent_request: Option<crate::agent::llm::types::CompactionParentRequest>,
    pub measured_output_tokens_reserve: usize,
    pub resume_completion_after_compact: bool,
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

/// Attempt preflight compaction using resume-fit split selection.
///
/// Returns `Ok(true)` when a compact request was emitted. Does not treat
/// "compaction input fits" as success unless the candidate also passes the
/// post-compact resume budget check above.
pub(crate) async fn try_trigger_preflight_compaction(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    input: PreflightCompactionTriggerInput<'_>,
) -> Result<bool, String> {
    let PreflightCompactionTriggerInput {
        session_id,
        session_name,
        messages,
        parent_request,
        measured_output_tokens_reserve,
        resume_completion_after_compact,
    } = input;
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
    let current_context_limit =
        std::cmp::min(settings.max_input_context(), settings.model_max_limit);
    let effective_input_budget = crate::agent::llm::token_utils::calculate_effective_input_budget(
        current_context_limit,
        measured_output_tokens_reserve,
    );

    let mut system_prompt_tokens = 0;
    let mut tools_tokens = 0;
    if let Some(ref parent) = parent_request {
        if let Some(ref prompt) = parent.system_prompt {
            system_prompt_tokens = crate::agent::llm::token_utils::estimate_text_tokens(prompt);
        }
        if let Some(ref tools) = parent.available_tools {
            if let Ok(json) = serde_json::to_string(tools) {
                tools_tokens = crate::agent::llm::token_utils::estimate_text_tokens(&json);
            }
        }
    } else {
        system_prompt_tokens = 2000;
        tools_tokens = 1500;
    }

    let compaction_limit = effective_input_budget
        .saturating_sub(system_prompt_tokens)
        .saturating_sub(tools_tokens)
        .saturating_sub(COMPACT_SUMMARY_PLACEHOLDER_TOKENS);

    let resume_fit_selection = select_resume_fit_compaction_split(
        messages,
        compact_context_record.as_ref(),
        system_prompt_tokens,
        tools_tokens,
        effective_input_budget,
        Some(compaction_limit),
    );
    let Some(resume_fit_selection) = resume_fit_selection else {
        log_preflight_split_boundary(session_id, messages, 0, "split_idx_zero");
        log::warn!(
            "⏭️ Preflight compaction skipped (no resume-fit split): session={}",
            session_id
        );
        return Ok(false);
    };

    let checkpoint_seed_split_idx = resume_fit_selection
        .checkpoint_seed_split_idx
        .unwrap_or(resume_fit_selection.chosen_split_idx);
    let preferred_split_idx = resume_fit_selection.chosen_split_idx;

    if preferred_split_idx == 0 {
        log_preflight_split_boundary(session_id, messages, 0, "split_idx_zero");
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
            preferred_split_idx,
        )
    {
        log::info!(
            "⏭️ Preflight compaction skipped (same tail): session={}, tail={}, chosen_split_idx={}, message_count={}, last_compacted_tail={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?"),
            preferred_split_idx,
            messages.len(),
            last_compacted_tail.as_deref().unwrap_or("?")
        );
        return Ok(false);
    }

    let started_at_ms = chrono::Utc::now().timestamp_millis();
    let initial_retry_attempt = compaction.retry_attempt().await;
    let initial_recovery_phase = compaction.recovery_phase().await;
    let mut split_candidates = build_resume_fit_split_candidates(
        messages,
        compact_context_record.as_ref(),
        Some(checkpoint_seed_split_idx),
    );
    // Ensure the resume-fit preferred split is tried first among deep→shallow order.
    if let Some(pos) = split_candidates
        .iter()
        .position(|&idx| idx == preferred_split_idx)
    {
        let preferred = split_candidates.remove(pos);
        split_candidates.insert(0, preferred);
    } else {
        split_candidates.insert(0, preferred_split_idx);
    }

    if split_candidates.is_empty() {
        return Err(format!(
            "No ownership-safe compaction split candidates available for session {}.",
            session_id
        ));
    }

    log::info!(
        "🪜 Preflight compaction split candidates: session={}, checkpoint_seed_split_idx={}, preferred_split_idx={}, projected_resume_tokens={}, effective_input_budget={}, candidates={:?}",
        session_id,
        checkpoint_seed_split_idx,
        preferred_split_idx,
        resume_fit_selection.projected_resume_tokens,
        effective_input_budget,
        split_candidates
    );

    let mut prepared_attempt = None;
    let mut chosen_split_idx = preferred_split_idx;
    for split_idx in split_candidates
        .iter()
        .copied()
        .take(MAX_COMPACTION_SPLIT_BACKOFF_ATTEMPTS)
    {
        let projected_resume = estimate_post_compact_resume_tokens(
            messages,
            split_idx,
            system_prompt_tokens,
            tools_tokens,
            COMPACT_SUMMARY_PLACEHOLDER_TOKENS,
        );
        if projected_resume >= effective_input_budget {
            log::info!(
                "⏭️ Skipping split that cannot fit post-compact resume: session={}, split_idx={}, projected_resume_tokens={}, effective_input_budget={}",
                session_id,
                split_idx,
                projected_resume,
                effective_input_budget
            );
            continue;
        }

        match prepare_compaction_request_with_recovery_ladder(
            active_sessions,
            PrepareCompactionRequestInput {
                session_id,
                session_name,
                messages,
                split_idx,
                measured_output_tokens_reserve,
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
        .await
        {
            Ok(Some(attempt)) => {
                prepared_attempt = Some(attempt);
                chosen_split_idx = split_idx;
                break;
            }
            Ok(None) => {
                log::info!(
                    "⏭️ Preflight compaction prepare returned no-op for split: session={}, split_idx={}",
                    session_id,
                    split_idx
                );
            }
            Err(error) => {
                // Compaction-input fitting failed for this resume-fit split; try the next
                // shallower ownership-safe candidate instead of aborting preflight.
                log::warn!(
                    "⚠️ Preflight compaction prepare failed for split, trying next candidate: session={}, split_idx={}, error={}",
                    session_id,
                    split_idx,
                    error
                );
            }
        }
    }

    if prepared_attempt.is_none() {
        if let Some(recovery_plan) = derive_tail_recompaction_recovery_plan(
            messages,
            compact_context_record.as_ref(),
            preferred_split_idx,
        ) {
            let fallback_projected = estimate_post_compact_resume_tokens(
                messages,
                recovery_plan.fallback_split_idx,
                system_prompt_tokens,
                tools_tokens,
                COMPACT_SUMMARY_PLACEHOLDER_TOKENS,
            );
            if fallback_projected < effective_input_budget {
                log::warn!(
                    "🧯 Forcing tail re-compaction recovery after no-op incremental payload: session={}, preferred_split_idx={}, fallback_split_idx={}, compacted_to_idx={}, first_delta_message_idx={}, latest_request_start_idx={}, message_count={}",
                    session_id,
                    preferred_split_idx,
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
                        measured_output_tokens_reserve,
                        parent_request: parent_request.clone(),
                        compact_context_record: compact_context_record.clone(),
                        started_at_ms,
                        resume_completion_after_compact,
                        recovery_phase:
                            crate::agent::state::CompactionRecoveryPhase::OverflowRecovery,
                        retry_attempt: 0,
                    },
                    crate::agent::state::CompactionRecoveryPhase::OverflowRecovery,
                    0,
                )
                .await?;
                if prepared_attempt.is_some() {
                    chosen_split_idx = recovery_plan.fallback_split_idx;
                }
            }
        }
    }

    let Some(PreparedCompactionAttempt {
        prepared,
        recovery_phase,
        retry_attempt,
    }) = prepared_attempt
    else {
        log::info!(
            "⏭️ Preflight compaction skipped (no new delta beyond previous summary): session={}, tail={}, preferred_split_idx={}, message_count={}, last_compacted_tail={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?"),
            preferred_split_idx,
            messages.len(),
            last_compacted_tail.as_deref().unwrap_or("?")
        );
        return Ok(false);
    };

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
            compaction.set_current_request(compact_event.clone()).await;
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
            "⏸️ Preflight compaction triggered: session={}, to_id={}, checkpoint_seed_split_idx={}, chosen_split_idx={}, compacted_delta_count={}, reused_prior_summary={}, projected_resume_tokens={}, effective_input_budget={}, tail={}, started_at_ms={}",
            session_id,
            log_to_id,
            checkpoint_seed_split_idx,
            chosen_split_idx,
            prepared.compacted_delta_count,
            prepared.reused_prior_summary,
            estimate_post_compact_resume_tokens(
                messages,
                chosen_split_idx,
                system_prompt_tokens,
                tools_tokens,
                COMPACT_SUMMARY_PLACEHOLDER_TOKENS,
            ),
            effective_input_budget,
            current_tail_id.as_deref().unwrap_or("?"),
            prepared.started_at_ms
        );
    } else {
        log::info!(
            "🧰 Manual compaction triggered: session={}, to_id={}, checkpoint_seed_split_idx={}, chosen_split_idx={}, compacted_delta_count={}, reused_prior_summary={}, projected_resume_tokens={}, effective_input_budget={}, tail={}, started_at_ms={}",
            session_id,
            log_to_id,
            checkpoint_seed_split_idx,
            chosen_split_idx,
            prepared.compacted_delta_count,
            prepared.reused_prior_summary,
            estimate_post_compact_resume_tokens(
                messages,
                chosen_split_idx,
                system_prompt_tokens,
                tools_tokens,
                COMPACT_SUMMARY_PLACEHOLDER_TOKENS,
            ),
            effective_input_budget,
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
    let measured_output_tokens_reserve =
        crate::agent::llm::token_utils::derive_measured_output_tokens_reserve(
            &merged_messages,
            None,
        );
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        PreflightCompactionTriggerInput {
            session_id,
            session_name: &session_name,
            messages: &merged_messages,
            parent_request: None,
            measured_output_tokens_reserve,
            resume_completion_after_compact: true,
        },
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
    let measured_output_tokens_reserve =
        crate::agent::llm::token_utils::derive_measured_output_tokens_reserve(
            &merged_messages,
            None,
        );
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        PreflightCompactionTriggerInput {
            session_id,
            session_name: &session_name,
            messages: &merged_messages,
            parent_request: None,
            measured_output_tokens_reserve,
            resume_completion_after_compact: false,
        },
    )
    .await
}
