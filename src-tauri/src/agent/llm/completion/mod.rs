pub(crate) mod compaction;
pub(crate) mod context;
pub(crate) mod request;

pub use compaction::{
    find_preflight_compaction_split_index, should_skip_same_tail_compaction,
    should_trigger_background_compaction, trigger_post_response_compaction_if_needed,
    trigger_preflight_compaction_for_session,
};
pub use context::{
    resolve_context_management_settings, uses_compaction_strategy, ContextManagementSettings,
};
pub use request::{
    build_compact_context_selection_options, build_compact_summary_text,
    merge_consecutive_user_messages, request_llm_completion,
};

// Crate-internal re-exports for intra-module visibility
pub(crate) use context::load_context_management_settings;
