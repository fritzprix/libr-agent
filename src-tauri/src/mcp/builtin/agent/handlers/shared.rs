use serde_json::Value;

use super::super::utils::build_agent_tool_data;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::{MCPContent, MCPResult};

pub(super) fn read_optional_string(args: &Value, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        Some(value) if value.is_null() => Ok(None),
        Some(value) => value
            .as_str()
            .map(|text| Some(text.to_string()))
            .ok_or_else(|| format!("Argument '{}' must be a string", key)),
        None => Ok(None),
    }
}

fn extract_result_text(result: &MCPResult) -> Option<String> {
    result
        .content
        .as_ref()?
        .iter()
        .find_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
}

pub(super) fn normalize_agent_config_result(
    mut result: MCPResult,
    tool_name: &str,
    next_actions: Vec<Value>,
) -> MCPResult {
    if result.is_error == Some(true) {
        return result;
    }

    let message =
        extract_result_text(&result).unwrap_or_else(|| format!("{} completed.", tool_name));
    let existing = result.structured_content.take();
    let resource_id = existing
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|object| object.get("id"))
        .and_then(|value| value.as_str());

    let mut data = build_agent_tool_data(
        tool_name,
        "agentConfig",
        resource_id,
        &message,
        "success",
        next_actions,
    );

    match existing {
        Some(Value::Object(object)) => {
            for (key, value) in object {
                data.insert(key, value);
            }
        }
        Some(value) => {
            data.insert("data".to_string(), value);
        }
        None => {}
    }

    result.structured_content = Some(Value::Object(data));
    result
}

pub(super) fn caller_session_not_found_result(session_id: &str) -> MCPResult {
    guided_error(
        ErrorCategory::ResourceNotFound,
        format!("Caller session '{}' not found", session_id),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Resume the parent/root session and retry the operation".to_string(),
        "Use agent__listAgents(type=\"sessions\") to inspect delegated sessions if needed"
            .to_string(),
        "The caller session may have been terminated or expired".to_string(),
    ])
    .to_mcp_result()
}

pub(super) fn missing_explicit_org_result() -> MCPResult {
    guided_error(
        ErrorCategory::InvalidInput,
        "No explicit org is associated with the current session. Call agent__createOrg first."
            .to_string(),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Use agent__createOrg(name=\"...\") from the root session first".to_string(),
        "Or pass orgId explicitly when querying a known explicit org".to_string(),
    ])
    .to_mcp_result()
}

pub(super) fn invalid_explicit_org_result(org_id: &str) -> MCPResult {
    guided_error(
        ErrorCategory::InvalidState,
        format!("Explicit org '{}' is missing a root session", org_id),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        "Use agent__createOrg(name=\"...\") again from the root session if the org lineage was reset"
            .to_string(),
        "Use agent__listAgents(type=\"sessions\") to inspect the current delegated lineage".to_string(),
    ])
    .to_mcp_result()
}
