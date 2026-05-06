pub(crate) mod compaction;
pub(crate) mod context;
pub(crate) mod orchestration;
pub(crate) mod request;

pub use compaction::{
    build_compaction_preservation_hints, fit_compaction_request_messages_to_limit,
    preview_background_compaction_selection, preview_preflight_compaction_selection,
    should_skip_same_tail_compaction, should_trigger_background_compaction,
    should_trigger_post_response_compaction, trigger_manual_compaction_for_session,
    trigger_post_response_compaction_if_needed, trigger_preflight_compaction_for_session,
    CompactionPreservationHints, CompactionSelectionPreview,
};
pub use context::{
    resolve_context_management_settings, uses_compaction_strategy, ContextManagementSettings,
};
pub use orchestration::request_llm_completion_with_recovery;
pub use request::{
    build_compact_context_selection_options, build_compact_summary_message_for_messages,
    build_compact_summary_text, merge_consecutive_user_messages, normalize_request_messages,
    request_llm_completion, resolve_preserved_calibration_ratio,
};

// Crate-internal re-exports for intra-module visibility
pub(crate) use context::load_context_management_settings;
