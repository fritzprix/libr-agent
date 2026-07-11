//! # Resume-fit compaction split contract
//!
//! Normative docs: `docs/specs/message-compaction.md` §5.2.
//!
//! ## Meaning of `split_idx`
//!
//! ```text
//! messages[0 .. split_idx)  -> compact into summary (prefix)
//! messages[split_idx ..]    -> retain as live tail for the next completion
//! ```
//!
//! `split_idx` is **not** "how many messages to send to the compaction LLM".
//!
//! ## Selection order (must not regress)
//!
//! 1. Build ownership-safe candidates (no orphan tool results in the retained tail).
//! 2. Prompt-token checkpoints may **seed** candidates only.
//! 3. Estimate post-compact resume tokens for each candidate.
//! 4. Choose the **deepest** candidate whose projected resume fits
//!    `effective_input_budget`.
//! 5. Compaction-input fitting may shrink the summarizer payload later, but must
//!    never move the chosen resume boundary (`to_id` / `split_idx`).
//!
//! ## Forbidden regression
//!
//! Accepting a shallow checkpoint-seeded split because the compaction *input*
//! fits, while leaving an oversized live tail that makes resume fail with
//! `INVALID_CONTEXT_STATE`.

use crate::models::chat::Message;
use crate::repositories::CompactContextRecord;

/// Placeholder budget reserved for the injected compact-summary message on resume.
/// Matches the safety margin subtracted when computing `compaction_limit` in trigger/orchestration.
pub(super) const COMPACT_SUMMARY_PLACEHOLDER_TOKENS: usize = 1500;

/// Conservative multiplier aligned with preflight token gating (`token_utils`).
const RESUME_FIT_SAFETY_MULTIPLIER: f64 = 1.05;

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

/// Result of resume-fit split selection.
///
/// - `chosen_split_idx`: committed compact/retain boundary (deepest fit).
/// - `checkpoint_seed_split_idx`: optional shallow seed from prompt-token window;
///   diagnostic only — must not override a deeper resume-fit choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeFitSplitSelection {
    pub chosen_split_idx: usize,
    pub checkpoint_seed_split_idx: Option<usize>,
    pub projected_resume_tokens: usize,
    pub effective_input_budget: usize,
}

pub(super) fn build_compaction_selection_preview(
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
    build_compaction_selection_preview(
        messages,
        find_preflight_compactable_end_exclusive(messages, None, None),
    )
}

pub(super) fn derive_tail_recompaction_recovery_plan(
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

pub(super) fn find_compaction_delta_start_index(
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

fn retained_tail_has_orphan_tool_messages(messages: &[Message], split_idx: usize) -> bool {
    use std::collections::HashSet;

    let split_idx = split_idx.min(messages.len());
    let tail = &messages[split_idx..];
    let tail_owner_ids = tail
        .iter()
        .filter(|message| message.role == "assistant")
        .flat_map(|message| {
            message
                .tool_calls
                .iter()
                .flatten()
                .map(|tool_call| tool_call.id.clone())
        })
        .collect::<HashSet<_>>();

    tail.iter().any(|message| {
        message.role == "tool"
            && message
                .tool_call_id
                .as_ref()
                .is_some_and(|tool_call_id| !tail_owner_ids.contains(tool_call_id))
    })
}

fn find_next_ownership_safe_split(
    messages: &[Message],
    delta_start_idx: usize,
    split_idx: usize,
) -> Option<usize> {
    let starting_split_idx = split_idx.max(delta_start_idx.saturating_add(1));

    (starting_split_idx..=messages.len()).find(|candidate_split_idx| {
        !retained_tail_has_orphan_tool_messages(messages, *candidate_split_idx)
    })
}

/// Project the next normal completion size after compacting `messages[..split_idx)`.
///
/// Contract: selection and preflight gating must agree that resume fitness is
/// measured on **summary + retained tail + system + tools**, not on compaction
/// request size.
pub(super) fn estimate_post_compact_resume_tokens(
    messages: &[Message],
    split_idx: usize,
    system_prompt_tokens: usize,
    tools_tokens: usize,
    summary_placeholder_tokens: usize,
) -> usize {
    let split_idx = split_idx.min(messages.len());
    let tail_tokens: usize = messages[split_idx..]
        .iter()
        .map(crate::agent::llm::token_utils::estimate_tokens_bpe)
        .sum();
    let full_estimate =
        summary_placeholder_tokens + tail_tokens + system_prompt_tokens + tools_tokens;
    (full_estimate as f64 * RESUME_FIT_SAFETY_MULTIPLIER).ceil() as usize
}

pub fn estimate_post_compact_resume_tokens_for_testing(
    messages: &[Message],
    split_idx: usize,
    system_prompt_tokens: usize,
    tools_tokens: usize,
    summary_placeholder_tokens: usize,
) -> usize {
    estimate_post_compact_resume_tokens(
        messages,
        split_idx,
        system_prompt_tokens,
        tools_tokens,
        summary_placeholder_tokens,
    )
}

fn is_ownership_safe_split(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    split_idx: usize,
) -> bool {
    let delta_start_idx = find_compaction_delta_start_index(messages, compact_context_record);
    split_idx > delta_start_idx
        && split_idx <= messages.len()
        && !retained_tail_has_orphan_tool_messages(messages, split_idx)
}

/// Build ownership-safe split candidates ordered deep → shallow (larger split first).
///
/// Deep splits compact more history and leave a smaller live tail for resume.
/// Checkpoint seeds are included but must not reorder this deep-first preference
/// away from resume fitness.
pub(super) fn build_resume_fit_split_candidates(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    checkpoint_seed_split_idx: Option<usize>,
) -> Vec<usize> {
    use std::collections::HashSet;

    let delta_start_idx = find_compaction_delta_start_index(messages, compact_context_record);
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    let mut push_candidate = |split_idx: usize| {
        if !is_ownership_safe_split(messages, compact_context_record, split_idx) {
            return;
        }
        if seen.insert(split_idx) {
            candidates.push(split_idx);
        }
    };

    // Prefer preserving the latest external-request seed block when possible.
    if let Some(latest_request_start) =
        super::find_latest_external_request_seed_block_start(messages)
    {
        push_candidate(latest_request_start);
    }

    if let Some(seed) = checkpoint_seed_split_idx {
        push_candidate(seed);
    }

    for (idx, message) in messages.iter().enumerate().rev() {
        if idx < delta_start_idx || message.prompt_tokens_value().is_none() {
            continue;
        }
        push_candidate(idx + 1);
    }

    // Deepest ownership-safe fallback: compact everything before an unresolved tool chain.
    let unresolved_split = delta_start_idx
        + crate::agent::llm::context_selector::find_compaction_split_index(
            &messages[delta_start_idx..],
        );
    push_candidate(unresolved_split);
    push_candidate(messages.len());

    // Deep → shallow so the first resume-fit candidate maximizes compacted history.
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    candidates
}

pub(super) fn build_checkpoint_backoff_split_candidates(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    initial_split_idx: usize,
) -> Vec<usize> {
    build_resume_fit_split_candidates(messages, compact_context_record, Some(initial_split_idx))
}

pub fn build_checkpoint_backoff_split_candidates_for_testing(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    initial_split_idx: usize,
) -> Vec<usize> {
    build_checkpoint_backoff_split_candidates(messages, compact_context_record, initial_split_idx)
}

/// Pick the deepest ownership-safe split whose projected post-compact resume fits budget.
///
/// `candidates` must already be ordered deep → shallow; the first fit wins.
pub(super) fn find_resume_fit_split_idx(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    candidates: &[usize],
    system_prompt_tokens: usize,
    tools_tokens: usize,
    effective_input_budget: usize,
    summary_placeholder_tokens: usize,
) -> Option<usize> {
    for &split_idx in candidates {
        if !is_ownership_safe_split(messages, compact_context_record, split_idx) {
            continue;
        }
        let projected = estimate_post_compact_resume_tokens(
            messages,
            split_idx,
            system_prompt_tokens,
            tools_tokens,
            summary_placeholder_tokens,
        );
        if projected < effective_input_budget {
            return Some(split_idx);
        }
    }
    None
}

pub fn find_resume_fit_split_idx_for_testing(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    candidates: &[usize],
    system_prompt_tokens: usize,
    tools_tokens: usize,
    effective_input_budget: usize,
    summary_placeholder_tokens: usize,
) -> Option<usize> {
    find_resume_fit_split_idx(
        messages,
        compact_context_record,
        candidates,
        system_prompt_tokens,
        tools_tokens,
        effective_input_budget,
        summary_placeholder_tokens,
    )
}

/// Select a compaction split that makes the post-compact resume prompt fit.
///
/// # Contract
///
/// Prefer the deepest ownership-safe split with
/// `projected_resume_tokens < effective_input_budget`.
/// Checkpoint window seeds are advisory only.
///
/// Falls back to the deepest ownership-safe candidate (preserving latest request when
/// possible) when no candidate projects under budget — caller may still fail hard later.
pub(super) fn select_resume_fit_compaction_split(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    system_prompt_tokens: usize,
    tools_tokens: usize,
    effective_input_budget: usize,
    compaction_limit: Option<usize>,
) -> Option<ResumeFitSplitSelection> {
    if messages.is_empty() {
        return None;
    }

    let checkpoint_seed_split_idx = compaction_limit.and_then(|limit| {
        find_prompt_checkpoint_compactable_end_exclusive(messages, compact_context_record, limit)
    });

    let candidates = build_resume_fit_split_candidates(
        messages,
        compact_context_record,
        checkpoint_seed_split_idx,
    );
    if candidates.is_empty() {
        return None;
    }

    let chosen_split_idx = find_resume_fit_split_idx(
        messages,
        compact_context_record,
        &candidates,
        system_prompt_tokens,
        tools_tokens,
        effective_input_budget,
        COMPACT_SUMMARY_PLACEHOLDER_TOKENS,
    )
    .or_else(|| {
        // Deepest candidate that still preserves the latest external request when possible.
        let latest_request_floor =
            super::find_latest_external_request_seed_block_start(messages).unwrap_or(0);
        candidates
            .iter()
            .copied()
            .find(|&split_idx| split_idx >= latest_request_floor)
            .or_else(|| candidates.first().copied())
    })?;

    let projected_resume_tokens = estimate_post_compact_resume_tokens(
        messages,
        chosen_split_idx,
        system_prompt_tokens,
        tools_tokens,
        COMPACT_SUMMARY_PLACEHOLDER_TOKENS,
    );

    Some(ResumeFitSplitSelection {
        chosen_split_idx,
        checkpoint_seed_split_idx,
        projected_resume_tokens,
        effective_input_budget,
    })
}

pub fn select_resume_fit_compaction_split_for_testing(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    system_prompt_tokens: usize,
    tools_tokens: usize,
    effective_input_budget: usize,
    compaction_limit: Option<usize>,
) -> Option<ResumeFitSplitSelection> {
    select_resume_fit_compaction_split(
        messages,
        compact_context_record,
        system_prompt_tokens,
        tools_tokens,
        effective_input_budget,
        compaction_limit,
    )
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
        if preserve_relative_idx > 0 {
            return find_next_ownership_safe_split(
                messages,
                delta_start_idx,
                delta_start_idx + preserve_relative_idx,
            );
        }
    }

    find_next_ownership_safe_split(
        messages,
        delta_start_idx,
        delta_start_idx + latest_checkpoint_relative_idx + 1,
    )
}

/// True when there is an ownership-safe split whose projected post-compact resume fits.
///
/// Despite the historical name, this is a **resume-fit** gate, not a "checkpoint
/// must exist" gate. Checkpoints seed candidates; missing checkpoints alone must
/// not force `INVALID_CONTEXT_STATE` when a resume-fit split still exists.
///
/// `current_context_limit` is the message-side compaction budget (sys/tools already
/// subtracted), matching the value computed in trigger/orchestration.
pub fn has_prompt_checkpoint_compaction_target(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    current_context_limit: usize,
) -> bool {
    select_resume_fit_compaction_split(
        messages,
        compact_context_record,
        0,
        0,
        current_context_limit.saturating_add(COMPACT_SUMMARY_PLACEHOLDER_TOKENS),
        Some(current_context_limit),
    )
    .is_some_and(|selection| {
        selection.projected_resume_tokens < selection.effective_input_budget
            && selection.chosen_split_idx
                > find_compaction_delta_start_index(messages, compact_context_record)
    })
}

pub(super) fn find_preflight_compactable_end_exclusive(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    current_context_limit: Option<usize>,
) -> usize {
    if messages.is_empty() {
        return 0;
    }

    if let Some(limit) = current_context_limit {
        // Callers pass the message-side compaction budget (sys/tools already subtracted).
        // Reconstruct an effective resume budget by re-adding the summary placeholder.
        if let Some(selection) = select_resume_fit_compaction_split(
            messages,
            compact_context_record,
            0,
            0,
            limit.saturating_add(COMPACT_SUMMARY_PLACEHOLDER_TOKENS),
            Some(limit),
        ) {
            return selection.chosen_split_idx;
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
