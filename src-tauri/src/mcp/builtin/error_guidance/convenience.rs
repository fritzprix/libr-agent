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

/// Create a guided error for a missing agent configuration during `agent__spawnSession`.
pub fn missing_agent_config_error(agent_id: &str) -> MCPResult {
    guided_error(
        ErrorCategory::ResourceNotFound,
        format!("Agent configuration '{}' not found", agent_id),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Call agent__listAgents(type=\"configs\") and copy a valid configuration template ID from the result"
            .to_string(),
        format!(
            "Do NOT guess, truncate, or typo-fix '{}'; re-copy the full configId from the list output",
            agent_id
        ),
        "Then call agent__spawnSession(configId=\"...\", task=\"...\") with that copied ID".to_string(),
    ])
    .to_mcp_result()
}

/// Create a guided error when an existing session ID was passed to `agent__spawnSession` instead of an agent config ID.
pub fn session_id_passed_as_agent_config_error(id: &str) -> MCPResult {
    guided_error(
        ErrorCategory::InvalidInput,
        format!(
            "'{}' is an existing Session ID, not an Agent Configuration ID",
            id
        ),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        format!(
            "To send tasks or follow-up instructions to this existing session, use agent__messageToSession(sessionId=\"{}\", message=\"...\")",
            id
        ),
        "Do NOT retry agent__spawnSession with this session ID".to_string(),
        "If you intended to spawn a brand new session, call agent__listAgents(type=\"configs\") first to copy a valid configuration template ID"
            .to_string(),
    ])
    .to_mcp_result()
}

/// Create a guided error when a delegated session id is missing or not accessible.
///
/// `accessible_count` is the number of sessions this caller can address
/// (same set as `agent__listAgents(type="sessions")` — direct children).
/// Prefer this over a bare "not found" so agents stop inventing session IDs.
pub fn missing_agent_session_error(session_id: &str, accessible_count: usize) -> MCPResult {
    if accessible_count == 0 {
        return guided_error(
            ErrorCategory::ResourceNotFound,
            format!(
                "No accessible delegated sessions. Session '{}' is not available to this caller.",
                session_id
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Do NOT retry agent__checkSession / agent__messageToSession with guessed or stale session IDs"
                .to_string(),
            "Proceed with your assigned deliverable, or re-read @teamwork/coordination/HANDOFF.md for sibling status"
                .to_string(),
            "Only call agent__checkSession after agent__listAgents(type=\"sessions\") returns a concrete sessionId"
                .to_string(),
        ])
        .to_mcp_result();
    }

    guided_error(
        ErrorCategory::ResourceNotFound,
        format!(
            "Session '{}' is not among the {} accessible delegated session(s) for this caller.",
            session_id, accessible_count
        ),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Use agent__listAgents(type=\"sessions\") and copy an exact sessionId from the list"
            .to_string(),
        format!(
            "Do NOT retry with sessionId '{}'; it is not in your accessible set",
            session_id
        ),
        "The session may have already finished, been stopped, or expired".to_string(),
    ])
    .to_mcp_result()
}
