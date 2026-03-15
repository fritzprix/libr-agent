use crate::mcp::builtin::error_guidance::{
  guided_error, missing_param_error, not_found_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::MCPContent;

fn extract_text(result: &crate::mcp::types::MCPResult) -> String {
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

fn extract_text_error_flag(result: &crate::mcp::types::MCPResult) -> Option<bool> {
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

  // We don't enforce exact wording, but we do enforce the contract:
  // - should clearly be an error
  // - should contain a Next Steps/Recovery section
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
    "Use listInteractable to see available elements first".to_string(),
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
fn not_found_error_mentions_the_missing_resource() {
  let r = not_found_error("Assistant", "asst_123", ToolGroup::Assistant);
  let text = extract_text(&r);

  assert_eq!(r.is_error, Some(true));
  assert_eq!(extract_text_error_flag(&r), Some(true));
  assert!(text.contains("asst_123"));
  assert!(text.contains("Next Steps") || text.contains("Recovery"));
}

#[test]
fn timeout_guided_error_is_not_marked_as_tool_error() {
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
fn internal_guided_error_is_not_marked_as_tool_error() {
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
