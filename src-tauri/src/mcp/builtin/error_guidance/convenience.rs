use super::builder::guided_error;
use super::categories::{ErrorCategory, ToolGroup};
use super::guidance::ErrorGuidance;
use crate::mcp::types::MCPResult;

/// Convenience functions for creating common errors with guidance
///
/// Create a "missing parameter" error with guidance
pub fn missing_param_error(param_name: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::MissingRequiredParam,
        format!("Missing required parameter: '{}'", param_name),
        tool_group,
    )
    .to_mcp_result()
}

/// Create a "resource not found" error with guidance
pub fn not_found_error(resource_type: &str, identifier: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::ResourceNotFound,
        format!("{} '{}' not found", resource_type, identifier),
        tool_group,
    )
    .to_mcp_result()
}

/// Create a "duplicate resource" error with guidance
pub fn duplicate_error(resource_type: &str, identifier: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::DuplicateResource,
        format!("{} '{}' already exists", resource_type, identifier),
        tool_group,
    )
    .to_mcp_result()
}

/// Create an "invalid input" error with guidance
pub fn invalid_input_error(message: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(ErrorCategory::InvalidInput, message, tool_group).to_mcp_result()
}

/// Create a "permission denied" error with guidance
pub fn permission_denied_error(resource: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::PermissionDenied,
        format!("Permission denied: {}", resource),
        tool_group,
    )
    .to_mcp_result()
}

/// Create an "operation failed" error with custom guidance
pub fn operation_failed_error(
    operation: &str,
    reason: &str,
    guidance: Vec<String>,
    tool_group: ToolGroup,
) -> MCPResult {
    ErrorGuidance::with_guidance(
        ErrorCategory::OperationFailed,
        format!("{} failed: {}", operation, reason),
        guidance,
        tool_group,
    )
    .to_mcp_result()
}

/// Create a guided error for a missing agent configuration during `startSession`.
pub fn missing_agent_config_error(agent_id: &str) -> MCPResult {
    guided_error(
        ErrorCategory::ResourceNotFound,
        format!("Agent configuration '{}' not found", agent_id),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Use list(type=\"configs\") to see available agent configurations".to_string(),
        format!("Verify '{}' exactly matches a listed agent ID", agent_id),
        "Retry startSession with a valid agentId copied from list(type=\"configs\")".to_string(),
    ])
    .to_mcp_result()
}

/// Create a guided error for a missing delegated agent session.
pub fn missing_agent_session_error(session_id: &str) -> MCPResult {
    guided_error(
        ErrorCategory::ResourceNotFound,
        format!("Agent session '{}' not found", session_id),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Use list(type=\"sessions\") to see active delegated sessions".to_string(),
        format!(
            "Verify sessionId '{}' matches one of the listed active session IDs",
            session_id
        ),
        "The session may have already finished, been stopped, or expired".to_string(),
    ])
    .to_mcp_result()
}
