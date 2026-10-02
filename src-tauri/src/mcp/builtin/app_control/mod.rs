//! Process-global app chrome control for remote automation (hero demos, CI, Cursor).
//!
//! Not registered on per-session `MCPServiceProxy` — served only via `POST /mcp/control`.
//! Tools are **generic UI primitives** (navigate, highlight, install preset, focus session, wait).
//! Scenario scripts (hero shot lists, etc.) live in docs / external agents — not as composite tools.

pub mod handlers;
pub mod tools;

use crate::mcp::types::{MCPContent, MCPResult};
use crate::mcp::MCPTool;
use handlers::{focus_session, highlight, install_preset, navigate, wait_ui, AppControlError};
use serde_json::Value;

/// Remote app chrome control surface.
#[derive(Debug, Default)]
pub struct AppControlServer;

impl AppControlServer {
    pub fn new() -> Self {
        Self
    }

    pub fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        let result = match tool_name {
            "app__navigate" | "navigate" => navigate(args),
            "app__highlight" | "highlight" => highlight(args),
            "app__install_preset" | "install_preset" => install_preset(args),
            "app__focus_session" | "focus_session" => focus_session(args),
            "app__wait_ui" | "wait_ui" => wait_ui(args).await,
            other => {
                return Err(format!(
                    "Unknown app-control tool: {other}. Available: app__navigate, app__highlight, app__install_preset, app__focus_session, app__wait_ui"
                ));
            }
        };

        result.map_err(|e: AppControlError| e.to_string())
    }
}

pub(crate) fn text_result(message: impl Into<String>) -> MCPResult {
    MCPResult {
        content: Some(vec![MCPContent::Text {
            text: message.into(),
        }]),
        structured_content: None,
        is_error: Some(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lists_expected_tools() {
        let names: Vec<_> = AppControlServer::new()
            .tools()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert!(names.contains(&"app__navigate".to_string()));
        assert!(names.contains(&"app__install_preset".to_string()));
        assert!(!names.iter().any(|n| n.contains("run_hero")));
        assert_eq!(names.len(), 5);
    }

    #[tokio::test]
    async fn install_preset_requires_name() {
        let err = AppControlServer::new()
            .call_tool("app__install_preset", json!({}))
            .await
            .expect_err("missing name");
        assert!(err.contains("name"), "{err}");
    }

    #[tokio::test]
    async fn navigate_requires_path() {
        let err = AppControlServer::new()
            .call_tool("app__navigate", json!({}))
            .await
            .expect_err("missing path");
        assert!(err.contains("path"), "{err}");
    }

    #[tokio::test]
    async fn navigate_rejects_protocol_relative_path() {
        let err = AppControlServer::new()
            .call_tool("app__navigate", json!({"path": "//evil.example"}))
            .await
            .expect_err("protocol-relative path");
        assert!(err.contains("in-app"), "{err}");
    }
}
