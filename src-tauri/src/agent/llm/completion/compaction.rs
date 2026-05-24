mod hints;
mod instruction;
mod payload;
mod trigger;

pub use hints::{build_compaction_preservation_hints, CompactionPreservationHints};
pub use payload::{
    apply_compaction_retry_budget, build_overflow_recovery_compaction_messages,
    fit_compaction_request_messages_to_limit,
};
pub(crate) use trigger::try_trigger_preflight_compaction;
pub use trigger::{
    advance_compaction_overflow_recovery_step_for_testing, preview_background_compaction_selection,
    preview_preflight_compaction_selection, should_skip_same_tail_compaction,
    trigger_manual_compaction_for_session, trigger_post_response_compaction_if_needed,
    trigger_preflight_compaction_for_session, CompactionSelectionPreview,
};

use super::context::uses_compaction_strategy;
use crate::models::chat::Message;

fn find_latest_external_request_block_start(messages: &[Message]) -> Option<usize> {
    let latest_request_idx = messages
        .iter()
        .rposition(Message::is_external_request_message)?;

    let mut block_start = latest_request_idx;
    while block_start > 0 && messages[block_start - 1].is_external_request_message() {
        block_start -= 1;
    }

    Some(block_start)
}

pub fn should_trigger_background_compaction(
    current_tokens: usize,
    safe_input_token_limit: usize,
    context_strategy: &str,
) -> bool {
    uses_compaction_strategy(context_strategy)
        && current_tokens
            > crate::agent::llm::token_utils::calculate_compact_threshold(safe_input_token_limit)
}

pub fn should_trigger_post_response_compaction(
    usage_total_tokens: usize,
    safe_input_token_limit: usize,
    context_strategy: &str,
) -> bool {
    should_trigger_background_compaction(
        usage_total_tokens,
        safe_input_token_limit,
        context_strategy,
    )
}
