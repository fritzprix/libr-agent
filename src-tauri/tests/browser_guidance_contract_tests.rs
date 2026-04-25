use tauri_mcp_agent_lib::mcp::builtin::error_guidance::{
    ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};

fn result_text(result: MCPResult) -> String {
    result
        .content
        .unwrap_or_default()
        .into_iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn browser_resource_not_found_guidance_matches_single_session_workflow() {
    let message = result_text(
        ErrorGuidance::new(
            ErrorCategory::ResourceNotFound,
            "No active browser session found for this agent",
            ToolGroup::Browser,
        )
        .to_mcp_result(),
    );

    assert!(message.contains("Use createSession to start a new browser session"));
    assert!(
        message.contains("Use getPageContent({}) to extract fresh content from the current page")
    );
    assert!(message.contains("Use listInteractable to inspect the current page before interacting"));
    assert!(!message.contains("listSessions"));
    assert!(!message.contains("session_id"));
}

#[test]
fn browser_operation_failed_guidance_uses_get_page_content_name() {
    let message = result_text(
        ErrorGuidance::new(
            ErrorCategory::OperationFailed,
            "Click element failed",
            ToolGroup::Browser,
        )
        .to_mcp_result(),
    );

    assert!(message.contains("Try getPageContent to view page structure"));
    assert!(!message.contains("extractWebContent"));
}

#[test]
fn browser_success_hint_next_steps_use_canonical_tool_names() {
    let create_session = SuccessHint::for_tool("createSession", ToolGroup::Browser).join("\n");
    assert!(create_session.contains("Use getPageContent to see the initial page"));
    assert!(!create_session.contains("extractWebContent"));

    let navigate = SuccessHint::for_tool("navigateToUrl", ToolGroup::Browser).join("\n");
    assert!(navigate.contains("Use getPageContent to see page content"));
    assert!(!navigate.contains("extractWebContent"));

    let get_page_content = SuccessHint::for_tool("getPageContent", ToolGroup::Browser).join("\n");
    assert!(get_page_content.contains("Use listInteractable to see interactive elements"));
    assert!(!get_page_content.contains("extractWebContent"));

    let list_interactable =
        SuccessHint::for_tool("listInteractable", ToolGroup::Browser).join("\n");
    assert!(list_interactable.contains("Use clickElement with the selector"));
    assert!(list_interactable.contains("Use getPageContent to see full page content"));
    assert!(!list_interactable.contains("index"));
    assert!(!list_interactable.contains("extractWebContent"));
}
