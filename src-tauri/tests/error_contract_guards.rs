use tauri_mcp_agent_lib::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, ToolGroup,
};
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

#[test]
fn missing_param_error_has_actionable_next_steps() {
    let r = missing_param_error("path", ToolGroup::Workspace);
    let text = extract_text(&r);

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
    assert!(text.contains("✗"));
    assert!(text.contains("Next Steps") || text.contains("Recovery"));
    assert!(text.contains("listInteractable"));
}

#[test]
fn not_found_error_mentions_the_missing_resource() {
    let r = not_found_error("Assistant", "asst_123", ToolGroup::Assistant);
    let text = extract_text(&r);

    assert!(text.contains("asst_123"));
    assert!(text.contains("Next Steps") || text.contains("Recovery"));
}
