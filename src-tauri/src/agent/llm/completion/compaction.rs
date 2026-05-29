mod hints;
mod instruction;
mod payload;
mod trigger;

use crate::models::chat::Message;
pub use hints::{build_compaction_preservation_hints, CompactionPreservationHints};
pub use payload::{
    apply_compaction_retry_budget, build_compaction_request_payload_for_testing,
    build_overflow_recovery_compaction_messages, fit_compaction_request_messages_to_limit,
    inspect_compaction_payload, CompactionPayloadDiagnostics, CompactionPayloadMessageDiagnostic,
    CompactionRequestPayloadPreview,
};
pub use trigger::{
    advance_compaction_overflow_recovery_step_for_testing,
    find_preflight_compactable_end_exclusive_for_testing, has_prompt_checkpoint_compaction_target,
    preview_preflight_compaction_selection, should_skip_same_tail_compaction,
    trigger_manual_compaction_for_session, trigger_preflight_compaction_for_session,
    CompactionSelectionPreview,
};
pub(crate) use trigger::{try_trigger_preflight_compaction, PreflightCompactionTriggerInput};
#[allow(dead_code)]
pub type TailRecompactionRecoveryPlan = trigger::TailRecompactionRecoveryPlan;

#[allow(dead_code)]
pub fn derive_tail_recompaction_recovery_plan_for_testing(
    messages: &[Message],
    compact_context_record: Option<&crate::repositories::CompactContextRecord>,
    split_idx: usize,
) -> Option<TailRecompactionRecoveryPlan> {
    trigger::derive_tail_recompaction_recovery_plan_for_testing(
        messages,
        compact_context_record,
        split_idx,
    )
}

fn find_latest_external_request_seed_block_start(messages: &[Message]) -> Option<usize> {
    let latest_request_idx = messages
        .iter()
        .rposition(Message::is_external_request_message)?;

    let mut block_start = latest_request_idx;
    while block_start > 0 && messages[block_start - 1].is_external_request_message() {
        block_start -= 1;
    }

    Some(block_start)
}
