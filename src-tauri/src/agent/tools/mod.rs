pub mod classification;
pub mod discovery;
pub mod execution;
pub mod media;
pub mod messages;
pub mod service_aliases;
pub mod spillover;

// Re-export all public API items for backward compatibility and clean ergonomics
pub use classification::{classify_tool_result, ToolResultAcceptance};
pub use discovery::collect_available_tools;
pub use execution::handle_tool_result;
pub use media::externalize_media_content_for_storage;
pub use messages::{
    convert_mcp_response_content, create_error_tool_result, create_tool_result_message,
    create_tool_result_message_with_content,
};
pub use service_aliases::{
    canonicalize_builtin_service_alias, extract_builtin_tool_ids, is_builtin_service_alias_enabled,
    runtime_allowed_builtin_service_aliases, runtime_allowed_builtin_service_aliases_from_value,
};
pub use spillover::{
    spill_oversized_tool_result_messages, tool_result_inline_limit_bytes,
    tool_result_preview_content_limit_bytes, tool_result_preview_headroom_bytes,
    TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
};
