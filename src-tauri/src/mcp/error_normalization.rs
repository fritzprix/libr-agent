use serde_json::json;

use crate::mcp::types::{MCPContent, MCPResult, ServiceInfo};

/// Normalized error category for external MCP failures.
///
/// These categories are intentionally stable and small; the goal is to provide
/// predictable recovery text for the agent regardless of transport or server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalMcpErrorCategory {
    Transport,
    Protocol,
    RemoteToolError,
    SessionExpired,
    Timeout,
    NotFound,
    InvalidInput,
    PermissionDenied,
    Internal,
}

impl ExternalMcpErrorCategory {
    fn as_label(self) -> &'static str {
        match self {
            Self::Transport => "Transport",
            Self::Protocol => "Protocol",
            Self::RemoteToolError => "RemoteToolError",
            Self::SessionExpired => "SessionExpired",
            Self::Timeout => "Timeout",
            Self::NotFound => "NotFound",
            Self::InvalidInput => "InvalidInput",
            Self::PermissionDenied => "PermissionDenied",
            Self::Internal => "Internal",
        }
    }
}

/// Build an agent-visible, tool-shaped error `MCPResult` for an external MCP tool failure.
///
/// Contract (text-first):
/// - Must include server + tool
/// - Must include Category + Cause
/// - Must include 1..N concrete recovery bullets
///
/// Note: UI grouping relies on `MCPContent::Text { is_error: Some(true) }`.
#[must_use]
pub fn external_tool_error_result(
    operation: &str,
    server_name: &str,
    tool_name: &str,
    category: ExternalMcpErrorCategory,
    cause: &str,
    recovery: Vec<String>,
) -> MCPResult {
    let mut text = String::new();

    // Keep this format stable: other parts of the system may snapshot-test it.
    text.push_str(&format!("✗ {operation}\n\n"));
    text.push_str(&format!("Source: External({server_name})\n"));
    text.push_str(&format!("Tool: {server_name}__{tool_name}\n"));
    text.push_str(&format!("Category: {}\n", category.as_label()));
    text.push_str(&format!("Cause: {cause}\n\n"));

    if recovery.is_empty() {
        text.push_str("Recovery:\n");
        text.push_str("- Use listTools for this server to confirm the tool name\n");
        text.push_str("- Verify the server is configured and reachable\n");
    } else {
        text.push_str("Recovery:\n");
        for step in &recovery {
            text.push_str(&format!("- {step}\n"));
        }
    }

    MCPResult {
        content: Some(vec![MCPContent::Text {
            text,
            is_error: Some(true),
        }]),
        structured_content: Some(json!({
          "error": {
            "operation": operation,
            "source": {
              "type": "external",
              "server": server_name,
              "tool": tool_name,
            },
            "category": category.as_label(),
            "cause": cause,
            "recovery": recovery,
          }
        })),
        is_error: Some(true),
    }
}

#[must_use]
pub fn external_service_info(server_name: &str, tool_name: &str) -> ServiceInfo {
    ServiceInfo {
        server_name: server_name.to_string(),
        tool_name: tool_name.to_string(),
        backend_type: "ExternalMCP".to_string(),
    }
}

/// Build an agent-visible, tool-shaped error `MCPResult` for a **builtin** MCP tool failure.
///
/// Identical contract to `external_tool_error_result` but labels the source as
/// `Builtin(server_name)` so agents can distinguish internal failures from
/// external server failures.
#[must_use]
pub fn builtin_tool_error_result(
    operation: &str,
    server_name: &str,
    tool_name: &str,
    category: ExternalMcpErrorCategory,
    cause: &str,
    recovery: Vec<String>,
) -> MCPResult {
    let mut text = String::new();

    text.push_str(&format!("✗ {operation}\n\n"));
    text.push_str(&format!("Source: Builtin({server_name})\n"));
    text.push_str(&format!("Tool: {server_name}__{tool_name}\n"));
    text.push_str(&format!("Category: {}\n", category.as_label()));
    text.push_str(&format!("Cause: {cause}\n\n"));

    if recovery.is_empty() {
        text.push_str("Recovery:\n");
        text.push_str("- Retry the operation\n");
        text.push_str("- Check application logs for details\n");
    } else {
        text.push_str("Recovery:\n");
        for step in &recovery {
            text.push_str(&format!("- {step}\n"));
        }
    }

    MCPResult {
        content: Some(vec![MCPContent::Text {
            text,
            is_error: Some(true),
        }]),
        structured_content: Some(json!({
          "error": {
            "operation": operation,
            "source": {
              "type": "builtin",
              "server": server_name,
              "tool": tool_name,
            },
            "category": category.as_label(),
            "cause": cause,
            "recovery": recovery,
          }
        })),
        is_error: Some(true),
    }
}

/// Classify a raw session API error string into a category and concrete recovery steps.
///
/// Used by builtin tools (e.g. `session_api`) to provide consistent, guided error
/// results matching the contract set by `external_tool_error_result`.
#[must_use]
pub fn categorize_session_api_error(err: &str) -> (ExternalMcpErrorCategory, Vec<String>) {
    // HTTP status-based classification (most specific first)
    if let Some(status) = extract_http_status(err) {
        return match status {
            400 => (
                ExternalMcpErrorCategory::InvalidInput,
                vec![
                    "Check the tool arguments match the expected schema".to_string(),
                    "Review the tool description for required parameter formats".to_string(),
                ],
            ),
            401 | 403 => (
                ExternalMcpErrorCategory::PermissionDenied,
                vec![
                    "The internal session API rejected the request with a permission error".to_string(),
                    "This may indicate a bug — please report it".to_string(),
                ],
            ),
            404 => (
                ExternalMcpErrorCategory::NotFound,
                vec![
                    "Verify the session ID is correct using getAgentStatus".to_string(),
                    "The session may have been terminated — use getChildAgents to list active sessions".to_string(),
                ],
            ),
            408 | 504 => (
                ExternalMcpErrorCategory::Timeout,
                vec![
                    "Increase the timeoutSeconds parameter".to_string(),
                    "Check the session status with getAgentStatus".to_string(),
                ],
            ),
            503 => (
                ExternalMcpErrorCategory::Transport,
                vec![
                    "The internal session HTTP server is temporarily unavailable — retry after a moment".to_string(),
                    "If the issue persists, restart the application".to_string(),
                ],
            ),
            500..=599 => (
                ExternalMcpErrorCategory::Internal,
                vec![
                    "Retry the operation — this is likely a transient internal error".to_string(),
                    "Check application logs for details".to_string(),
                ],
            ),
            _ => (
                ExternalMcpErrorCategory::Internal,
                vec!["Retry the operation".to_string()],
            ),
        };
    }

    // Non-HTTP error patterns
    if err.contains("request failed")
        || err.contains("connection refused")
        || err.contains("Failed to connect")
    {
        return (
            ExternalMcpErrorCategory::Transport,
            vec![
                "The internal session HTTP server may not be running — check the application state"
                    .to_string(),
                "Restart the application if this error persists".to_string(),
            ],
        );
    }

    if err.contains("timed out") || err.contains("timeout") {
        return (
            ExternalMcpErrorCategory::Timeout,
            vec![
                "Increase the timeoutSeconds parameter".to_string(),
                "Check the session status with getAgentStatus to see if it is still running"
                    .to_string(),
                "Use awaitAgent with a longer timeout to wait for slow sessions".to_string(),
            ],
        );
    }

    if err.contains("Invalid JSON") || err.contains("Failed to read response body") {
        return (
            ExternalMcpErrorCategory::Protocol,
            vec![
                "Retry the operation — this may be a transient parsing error".to_string(),
                "If the error repeats, this is likely a bug — please report it".to_string(),
            ],
        );
    }

    if err.contains("Missing") || err.contains("required") || err.contains("missing") {
        return (
            ExternalMcpErrorCategory::InvalidInput,
            vec![
                "Check the tool arguments — a required parameter is missing".to_string(),
                "Review the tool schema for required fields".to_string(),
            ],
        );
    }

    // Default fallback
    (
        ExternalMcpErrorCategory::Internal,
        vec![
            "Retry the operation".to_string(),
            "Check application logs for details".to_string(),
        ],
    )
}

/// Extract the HTTP status code from a `call_json` error string, if present.
///
/// Recognises the format emitted by `client::call_json`:
/// `"Request failed (NNN): cause"`
fn extract_http_status(err: &str) -> Option<u16> {
    // Pattern: "failed (NNN):"
    let marker = "failed (";
    let start = err.find(marker)? + marker.len();
    let end = err[start..].find(')')?;
    err[start..start + end].parse::<u16>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_tool_error_result_contains_contract_fields() {
        let r = external_tool_error_result(
            "Call External Tool",
            "filesystem",
            "read_file",
            ExternalMcpErrorCategory::Transport,
            "failed to spawn process",
            vec!["Restart the server".to_string()],
        );

        assert_eq!(r.is_error, Some(true));
        let content = r.content.expect("content");
        let MCPContent::Text { text, is_error } = content[0].clone() else {
            panic!("expected text");
        };

        assert_eq!(is_error, Some(true));
        assert!(text.contains("Source: External(filesystem)"));
        assert!(text.contains("Tool: filesystem__read_file"));
        assert!(text.contains("Category: Transport"));
        assert!(text.contains("Cause: failed to spawn process"));
        assert!(text.contains("Recovery:"));
        assert!(text.contains("- Restart the server"));
    }
}
