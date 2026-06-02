use crate::models::chat::Message;
use crate::repositories::CompactContextRecord;

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

pub(super) fn build_checkpoint_backoff_split_candidates(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    initial_split_idx: usize,
) -> Vec<usize> {
    use std::collections::HashSet;

    let delta_start_idx = find_compaction_delta_start_index(messages, compact_context_record);
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    let mut push_candidate = |split_idx: usize| {
        if split_idx <= delta_start_idx || split_idx > messages.len() {
            return;
        }
        if retained_tail_has_orphan_tool_messages(messages, split_idx) {
            return;
        }
        if seen.insert(split_idx) {
            candidates.push(split_idx);
        }
    };

    push_candidate(initial_split_idx);

    for (idx, message) in messages.iter().enumerate().rev() {
        if idx < delta_start_idx || message.prompt_tokens_value().is_none() {
            continue;
        }
        push_candidate(idx + 1);
    }

    candidates
}

pub fn build_checkpoint_backoff_split_candidates_for_testing(
    messages: &[Message],
    compact_context_record: Option<&CompactContextRecord>,
    initial_split_idx: usize,
) -> Vec<usize> {
    build_checkpoint_backoff_split_candidates(messages, compact_context_record, initial_split_idx)
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

pub(super) fn find_preflight_compactable_end_exclusive(
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
