pub(crate) mod compaction;
pub(crate) mod context;
pub(crate) mod request;

pub use compaction::{
    find_preflight_compaction_split_index, maybe_trigger_post_idle_compaction,
    should_trigger_background_compaction, trigger_preflight_compaction_for_session,
};
pub use context::{
    resolve_context_management_settings, uses_compaction_strategy, ContextManagementSettings,
};
pub use request::request_llm_completion;

// Crate-internal re-exports for intra-module visibility
pub(crate) use context::load_context_management_settings;
