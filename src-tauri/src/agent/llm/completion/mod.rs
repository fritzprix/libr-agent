pub(crate) mod compaction;
pub(crate) mod context;
pub(crate) mod orchestration;
pub(crate) mod request;

pub use compaction::{
    advance_compaction_overflow_recovery_step_for_testing, apply_compaction_retry_budget,
    build_checkpoint_backoff_split_candidates_for_testing, build_compaction_preservation_hints,
    build_compaction_request_payload_for_testing, build_overflow_recovery_compaction_messages,
    derive_tail_recompaction_recovery_plan_for_testing,
    find_preflight_compactable_end_exclusive_for_testing, fit_compaction_request_messages_to_limit,
    inspect_compaction_payload, preview_preflight_compaction_selection,
    should_skip_same_tail_compaction, trigger_manual_compaction_for_session,
    trigger_preflight_compaction_for_session, CompactionPayloadDiagnostics,
    CompactionPayloadMessageDiagnostic, CompactionPreservationHints,
    CompactionRequestPayloadPreview, CompactionSelectionPreview, TailRecompactionRecoveryPlan,
};
pub use context::{
    resolve_context_management_settings, uses_compaction_strategy, ContextManagementSettings,
};
pub use orchestration::request_llm_completion_with_recovery;
pub use request::{
    build_compact_context_selection_options, build_compact_summary_message,
    build_compact_summary_message_for_messages, build_compact_summary_text,
    merge_consecutive_user_messages, normalize_request_messages, request_llm_completion,
    resolve_preserved_calibration_ratio, summarize_recent_tool_calls,
};

// Crate-internal re-exports for intra-module visibility
pub(crate) use context::load_context_management_settings;
