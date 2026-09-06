//! Agent builtin MCP tool handlers.
//!
//! Shared helpers live in sibling modules; tool entrypoints remain in
//! `check_session`, `configs`, `orgs`, and `sessions`.

mod check_session;
mod check_session_results;
mod configs;
mod delegation;
mod enrichment;
mod orgs;
mod sessions;
mod shared;

pub use check_session::check_session;
pub use check_session_results::{
    build_paused_check_session_result_from_messages,
    build_terminal_check_session_result_from_messages,
};
pub use configs::{
    create_agent, list_agent_configs_for_test, list_agents_or_sessions,
    list_delegated_sessions_for_test, update_agent,
};
pub use delegation::{
    is_delegated_descendant_session, load_accessible_delegated_session, prepare_teamwork_workspace,
};
pub use enrichment::{
    append_check_session_context_to_message, apply_check_session_enrichment,
    check_session_enrichment_from_metadata, check_session_enrichment_from_metadata_with_caller,
    format_check_session_context_text, format_workspace_metadata_line,
    format_workspace_status_note, resolve_check_session_enrichment, CheckSessionEnrichment,
    WorkspaceRelation,
};
pub use orgs::{
    create_org, create_org_scaffold_preflight, existing_explicit_org_identity, get_org,
    inspect_teamwork_scaffold, TeamworkScaffoldStatus,
};
pub use sessions::{
    compact_session_context, delete_session, message_to_session,
    parse_message_to_session_wait_config, start_session, stop_session,
};
