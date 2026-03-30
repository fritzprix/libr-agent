use super::*;
use crate::mcp::types::MCPContent;

#[test]
fn test_error_guidance_formatting() {
    let error = ErrorGuidance::new(
        ErrorCategory::ResourceNotFound,
        "Session 'abc123' not found",
        ToolGroup::Browser,
    );

    let result = error.to_mcp_result();

    assert!(result.is_error == Some(true));

    if let Some(content) = result.content {
        if let Some(MCPContent::Text { text, .. }) = content.first() {
            assert!(text.contains("✗"));
            assert!(text.contains("💡 Next Steps:"));
            assert!(text.contains("Session 'abc123' not found"));
            assert!(text.contains("1. "));
        }
    }
}

#[test]
fn test_internal_error_guidance_is_informational() {
    let error = ErrorGuidance::new(
        ErrorCategory::InternalError,
        "Database connection dropped",
        ToolGroup::Knowledge,
    );

    let result = error.to_mcp_result();

    assert_eq!(result.is_error, Some(false));

    if let Some(content) = result.content {
        if let Some(MCPContent::Text { text, is_error }) = content.first() {
            assert_eq!(*is_error, None);
            assert!(text.contains("Notice:"));
            assert!(text.contains("💡 Next Steps:"));
        }
    }
}

#[test]
fn test_success_hint_formatting() {
    let hint = SuccessHint::new(
        "Todo created successfully",
        vec![
            "Use getCurrentState to see all todos".to_string(),
            "Use updateTodo(todoId=..., action='done') to mark as complete".to_string(),
        ],
    );

    let result = hint.to_mcp_result();

    assert!(result.is_error == Some(false));

    if let Some(content) = result.content {
        if let Some(MCPContent::Text { text, .. }) = content.first() {
            assert!(text.contains("✓"));
            assert!(text.contains("💡 Next:"));
            assert!(text.contains("Todo created successfully"));
        }
    }
}

#[test]
fn test_tool_group_isolation_browser() {
    let error = ErrorGuidance::new(
        ErrorCategory::ResourceNotFound,
        "Session not found",
        ToolGroup::Browser,
    );

    // Should suggest browser tools only
    assert!(error.guidance.iter().any(|g| g.contains("createSession")));
    assert!(!error.guidance.iter().any(|g| g.contains("addTodo"))); // Should not suggest planning tools
}

#[test]
fn test_tool_group_isolation_planning() {
    let error = ErrorGuidance::new(
        ErrorCategory::DuplicateResource,
        "Todo already exists",
        ToolGroup::Planning,
    );

    // Should suggest planning tools only
    assert!(error.guidance.iter().any(|g| g.contains("getCurrentState")));
    assert!(!error.guidance.iter().any(|g| g.contains("navigateToUrl"))); // Should not suggest browser tools
}

#[test]
fn test_guided_error_builder_uses_default_guidance() {
    let result = guided_error(
        ErrorCategory::ResourceNotFound,
        "Session 'abc123' not found",
        ToolGroup::Browser,
    )
    .to_mcp_result();

    assert!(result.is_error == Some(true));

    let content = result.content.expect("Expected MCPResult.content");
    let MCPContent::Text { text, .. } = content
        .first()
        .expect("Expected at least one content item")
        .clone()
    else {
        panic!("Expected Text content");
    };

    // Default guidance for (ResourceNotFound, Browser) includes createSession.
    assert!(text.contains("createSession"));
    assert!(text.contains("✗"));
    assert!(text.contains("💡 Next Steps:"));
}

#[test]
fn test_timeout_guided_error_builder_is_informational() {
    let result = guided_error(
        ErrorCategory::Timeout,
        "Waiting for browser session timed out",
        ToolGroup::Browser,
    )
    .to_mcp_result();

    assert_eq!(result.is_error, Some(false));

    let content = result.content.expect("Expected MCPResult.content");
    let MCPContent::Text { text, is_error } = content
        .first()
        .expect("Expected at least one content item")
        .clone()
    else {
        panic!("Expected Text content");
    };

    assert_eq!(is_error, None);
    assert!(text.contains("Notice:"));
    assert!(text.contains("💡 Next Steps:"));
}

#[test]
fn test_guided_error_builder_allows_override_guidance() {
    let result = guided_error(ErrorCategory::InvalidInput, "Bad input", ToolGroup::UI)
        .guidance(vec!["Use prompt_user with type='text'".to_string()])
        .to_mcp_result();

    let content = result.content.expect("Expected MCPResult.content");
    let MCPContent::Text { text, .. } = content
        .first()
        .expect("Expected at least one content item")
        .clone()
    else {
        panic!("Expected Text content");
    };

    // Should contain the override guidance, and should not need to match UI defaults.
    assert!(text.contains("1. Use prompt_user with type='text'"));
}
