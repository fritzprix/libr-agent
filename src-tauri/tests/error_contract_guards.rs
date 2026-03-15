use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::error_guidance::{
    guided_error, missing_agent_config_error, missing_agent_session_error, missing_param_error,
    not_found_error, ErrorCategory, ToolGroup,
};
use tauri_mcp_agent_lib::mcp::builtin::session_api::utils::handle_wait_timeout_result;
use tauri_mcp_agent_lib::mcp::builtin::ui::UiServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;

fn extract_text(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    let content = result
        .content
        .as_ref()
        .expect("expected MCPResult.content")
        .first()
        .expect("expected at least one MCPContent item");

    match content {
        MCPContent::Text { text, .. } => text.clone(),
        other => panic!("expected MCPContent::Text, got: {other:?}"),
    }
}

fn extract_text_error_flag(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> Option<bool> {
    let content = result
        .content
        .as_ref()
        .expect("expected MCPResult.content")
        .first()
        .expect("expected at least one MCPContent item");

    match content {
        MCPContent::Text { is_error, .. } => *is_error,
        other => panic!("expected MCPContent::Text, got: {other:?}"),
    }
}

#[test]
fn missing_param_error_has_actionable_next_steps() {
    let r = missing_param_error("path", ToolGroup::Workspace);
    let text = extract_text(&r);

    assert_eq!(r.is_error, Some(true));
    assert_eq!(extract_text_error_flag(&r), Some(true));
    assert!(text.contains("✗"));
    assert!(text.contains("Next Steps") || text.contains("Recovery"));
}

#[test]
fn guided_error_includes_next_steps_section() {
    let r = guided_error(
        ErrorCategory::InvalidInput,
        "Invalid selector",
        ToolGroup::Browser,
    )
    .with_guidance(vec![
        "Use listInteractable to see available elements first".to_string()
    ])
    .to_mcp_result();

    let text = extract_text(&r);
    assert_eq!(r.is_error, Some(true));
    assert_eq!(extract_text_error_flag(&r), Some(true));
    assert!(text.contains("✗"));
    assert!(text.contains("Next Steps") || text.contains("Recovery"));
    assert!(text.contains("listInteractable"));
}

#[test]
fn not_found_error_is_informational() {
    let r = not_found_error("Assistant", "asst_123", ToolGroup::Agent);
    let text = extract_text(&r);

    assert_eq!(r.is_error, Some(false));
    assert_eq!(extract_text_error_flag(&r), None);
    assert!(text.contains("Notice:"));
    assert!(text.contains("asst_123"));
    assert!(text.contains("Next Steps") || text.contains("Recovery"));
}

#[test]
fn missing_agent_config_error_suggests_listing_configs() {
    let r = missing_agent_config_error("exa");
    let text = extract_text(&r);

    assert_eq!(r.is_error, Some(false));
    assert_eq!(extract_text_error_flag(&r), None);
    assert!(text.contains("Agent configuration 'exa' not found"));
    assert!(text.contains("list(type=\"configs\")"));
    assert!(text.contains("Retry startSession with a valid agentId"));
}

#[test]
fn missing_agent_session_error_suggests_listing_sessions() {
    let r = missing_agent_session_error("sess_123");
    let text = extract_text(&r);

    assert_eq!(r.is_error, Some(false));
    assert_eq!(extract_text_error_flag(&r), None);
    assert!(text.contains("Agent session 'sess_123' not found"));
    assert!(text.contains("list(type=\"sessions\")"));
}

#[test]
fn timeout_guided_error_is_informational() {
    let r = guided_error(
        ErrorCategory::Timeout,
        "Command execution timeout after 30 seconds",
        ToolGroup::Workspace,
    )
    .to_mcp_result();
    let text = extract_text(&r);

    assert_eq!(r.is_error, Some(false));
    assert_eq!(extract_text_error_flag(&r), None);
    assert!(text.contains("Notice:"));
    assert!(text.contains("Next Steps") || text.contains("Recovery"));
}

#[test]
fn internal_guided_error_is_informational() {
    let r = guided_error(
        ErrorCategory::InternalError,
        "Database connection dropped",
        ToolGroup::Knowledge,
    )
    .to_mcp_result();
    let text = extract_text(&r);

    assert_eq!(r.is_error, Some(false));
    assert_eq!(extract_text_error_flag(&r), None);
    assert!(text.contains("Notice:"));
    assert!(text.contains("Next Steps") || text.contains("Recovery"));
}

#[test]
fn session_wait_timeout_is_converted_to_success_result() {
    let timeout_error = Err("HTTP 504 Gateway Timeout: request timed out".to_string());

    let result = handle_wait_timeout_result(timeout_error, "sess_123", 15, false)
        .expect_err("timeout should be converted into an MCPResult")
        .expect("timeout should not bubble as hard error");

    let text = extract_text(&result);

    assert_eq!(result.is_error, Some(false));
    assert_eq!(extract_text_error_flag(&result), Some(false));
    assert!(text.contains("timed out after 15s"));
    assert!(text.contains("checkSession(sessionId=\"sess_123\", wait=true)"));
    assert!(text.contains("list(type=\"sessions\")"));
}

#[tokio::test]
async fn ui_cancel_returns_informational_result() {
    let server = UiServer::new();

    let result = server
        .call_tool(
            "getUserAnswer",
            json!({
                "messageId": "msg_123",
                "cancelled": true
            }),
            None,
        )
        .await
        .expect("ui tool call should succeed");

    let text = extract_text(&result);

    assert_eq!(result.is_error, Some(false));
    assert_eq!(extract_text_error_flag(&result), None);
    assert_eq!(text, "User cancelled the prompt.");
}
