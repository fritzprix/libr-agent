pub mod compact;
pub mod context_selection;
pub mod formatting;
pub mod orchestration;

pub use compact::{
    build_compact_summary_message, build_compact_summary_message_for_messages,
    build_compact_summary_text,
};
pub use context_selection::{
    build_compact_context_selection_options, resolve_preserved_calibration_ratio,
    try_apply_lossy_main_request_fallback,
};
pub use formatting::{merge_consecutive_user_messages, normalize_request_messages};
pub use orchestration::request_llm_completion;
