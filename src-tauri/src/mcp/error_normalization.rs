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
