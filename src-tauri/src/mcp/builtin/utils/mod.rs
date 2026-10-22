//! Shared helpers for built-in MCP servers.

mod paths;
mod resource_response;
mod security;
mod sensitive_path_policy;
mod session_access;

pub use paths::{
    display_workspace_path, normalize_user_path, path_starts_with, relative_path_under_base,
    strip_windows_verbatim_prefix, workspace_paths_equivalent,
};
pub use resource_response::create_resource_response;
pub use security::{SecurityError, SecurityValidator};
pub use sensitive_path_policy::matches_sensitive_path_policy;
pub use session_access::{load_session_tool_access, SessionToolAccess};
