mod compact;
mod lifecycle;
mod message;
mod spawn;
mod workspace;

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;

pub use compact::compact_session_context;
pub use lifecycle::{delete_session, stop_session};
pub use message::{message_to_session, parse_message_to_session_wait_config};
pub use spawn::spawn_session;
pub use workspace::{
    is_inherit_workspace_override, org_child_inherited_docker_workspace,
    resolve_spawn_workspace_binding,
};

fn self_target_session_action_result(
    tool_name: &str,
    message: &str,
    guidance: Vec<String>,
) -> MCPResult {
    guided_error(
        ErrorCategory::InvalidState,
        message.to_string(),
        ToolGroup::Agent,
    )
    .with_guidance(
        std::iter::once(format!(
            "Use {} only for child or delegated sessions",
            tool_name
        ))
        .chain(guidance)
        .collect(),
    )
    .to_mcp_result()
}
