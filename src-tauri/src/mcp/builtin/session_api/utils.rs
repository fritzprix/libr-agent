use serde_json::Value;
use crate::mcp::types::{MCPContent, MCPResult};

pub fn success_result(text: String, data: Value) -> MCPResult {
    MCPResult {
        content: Some(vec![MCPContent::Text {
            text,
            is_error: None,
        }]),
        structured_content: Some(data),
        is_error: Some(false),
    }
}

pub fn read_required_string(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

pub fn resolve_parent_session_id(
    provided_parent: Option<&str>,
    caller_session_id: Option<&str>,
) -> Option<String> {
    match provided_parent
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) if value.eq_ignore_ascii_case("current") => {
            caller_session_id.map(str::to_string)
        }
        Some(value) => Some(value.to_string()),
        None => caller_session_id.map(str::to_string),
    }
}
