/// Error Guidance System for Built-in MCP Tools
///
/// This module provides a centralized error guidance system that ensures consistent,
/// actionable error messages across all built-in tools. It follows the best practices
/// documented in docs/guides/builtin-tool-best-practices.md.
///
/// Key principles:
/// - Every error includes visual markers (✗)
/// - Errors provide 2-3 actionable recovery steps
/// - Tool group isolation: Browser tools suggest browser tools, etc.
/// - Never expose internal state in error messages
/// - Consistent formatting across all tool groups
use crate::mcp::types::MCPResult;

/// Error categories for classification and guidance mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    // Input validation errors (user-fixable)
    MissingRequiredParam,
    InvalidInput,
    InvalidFormat,

    // State/resource errors (context-dependent)
    ResourceNotFound,
    DuplicateResource,
    InvalidState,
    NestingTooDeep,

    // Operation failures (may be transient)
    OperationFailed,
    Timeout,
    NetworkError,

    // System errors (escalation needed)
    InternalError,
    DatabaseError,
    PermissionDenied,
}

/// Tool group for isolation of tool suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGroup {
    Browser,
    Planning,
    Workspace,
    Assistant,
    ContentStore,
    Knowledge,
    Playbook,
    UI,
    McpManager,
    Bootstrap,
}

/// Structured error with guidance
pub struct ErrorGuidance {
    pub category: ErrorCategory,
    pub message: String,
    pub guidance: Vec<String>,
    pub tool_group: ToolGroup,
}

impl ErrorGuidance {
    /// Create a new error with guidance
    pub fn new(category: ErrorCategory, message: impl Into<String>, tool_group: ToolGroup) -> Self {
        let message = message.into();
        let guidance = Self::get_default_guidance(category, tool_group);

        Self {
            category,
            message,
            guidance,
            tool_group,
        }
    }

    /// Create an error with custom guidance steps
    pub fn with_guidance(
        category: ErrorCategory,
        message: impl Into<String>,
        guidance: Vec<String>,
        tool_group: ToolGroup,
    ) -> Self {
        Self {
            category,
            message: message.into(),
            guidance,
            tool_group,
        }
    }

    /// Convert to MCPResult
    pub fn to_mcp_result(&self) -> MCPResult {
        let guidance_text = self
            .guidance
            .iter()
            .enumerate()
            .map(|(i, step)| format!("{}. {}", i + 1, step))
            .collect::<Vec<_>>()
            .join("\n");

        let formatted_message = format!("✗ {}\n\n💡 Next Steps:\n{}", self.message, guidance_text);

        MCPResult::error(&formatted_message)
    }

    /// Get default guidance for an error category within a tool group
    fn get_default_guidance(category: ErrorCategory, tool_group: ToolGroup) -> Vec<String> {
        match (category, tool_group) {
            // Browser tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Browser) => vec![
                "Use createSession to start a new browser session".to_string(),
                "Use listSessions to see available sessions (if available)".to_string(),
                "Verify the session_id parameter is correct".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Browser) => vec![
                "Verify the URL format is valid (must include http:// or https://)".to_string(),
                "Check selector syntax matches CSS selector standards".to_string(),
                "Use listInteractable to see available elements first".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::Browser) => vec![
                "Try extractWebContent to view page structure".to_string(),
                "Use navigateToUrl to reload the page".to_string(),
                "Verify the target element is visible and interactable".to_string(),
            ],

            // Planning tool errors
            (ErrorCategory::DuplicateResource, ToolGroup::Planning) => vec![
                "Use a different title for the new item".to_string(),
                "Use checkTodo to modify the existing item".to_string(),
                "Use getCurrentState to see all existing items".to_string(),
            ],
            (ErrorCategory::ResourceNotFound, ToolGroup::Planning) => vec![
                "Use getCurrentState to see available todos".to_string(),
                "Verify the ID is correct and the item exists".to_string(),
                "Use getCurrentState to see the full planning state".to_string(),
                "Create as top-level todo instead".to_string(),
                "Attach to a different parent that has no parent".to_string(),
                "Use getCurrentState to see the current hierarchy".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Planning) => vec![
                "Ensure title is a non-empty string".to_string(),
                "Priority must be 'low', 'medium', or 'high'".to_string(),
                "Use getCurrentState to see existing todos for reference".to_string(),
            ],

            // Workspace tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Workspace) => vec![
                "Use listDirectory to see available files".to_string(),
                "Verify the file path is correct".to_string(),
                "Check if the file exists in the expected location".to_string(),
            ],
            (ErrorCategory::PermissionDenied, ToolGroup::Workspace) => vec![
                "Check file permissions with listDirectory".to_string(),
                "Ensure the path is within allowed directories".to_string(),
                "Verify you have read/write access to the target".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Workspace) => vec![
                "Verify the file path format is correct".to_string(),
                "Check that all required parameters are provided".to_string(),
                "Use listDirectory to see the correct path structure".to_string(),
            ],

            // Assistant tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Assistant) => vec![
                "Use listAssistants to see available assistants".to_string(),
                "Verify the assistant ID is correct".to_string(),
                "Use searchAssistant to find assistants by name".to_string(),
            ],
            (ErrorCategory::DuplicateResource, ToolGroup::Assistant) => vec![
                "Use a different name for the new assistant".to_string(),
                "Use updateAssistant to modify the existing one".to_string(),
                "Use listAssistants to see all assistants".to_string(),
            ],

            // Content Store tool errors
            (ErrorCategory::InvalidFormat, ToolGroup::ContentStore) => vec![
                "Ensure the file format is supported (PDF, HTML, markdown, code)".to_string(),
                "Check the file is not corrupted".to_string(),
                "Try a different file or format".to_string(),
            ],
            (ErrorCategory::ResourceNotFound, ToolGroup::ContentStore) => vec![
                "Use listContent to see available content".to_string(),
                "Verify the content ID is correct".to_string(),
                "Use keywordSimilaritySearch to find content by keywords".to_string(),
            ],

            // Knowledge tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Knowledge) => vec![
                "Use listKnowledge to see available knowledge entries".to_string(),
                "Use keywordSimilaritySearch to find entries by keyword".to_string(),
                "Verify the knowledge ID is correct".to_string(),
            ],

            // UI tool errors
            (ErrorCategory::InvalidInput, ToolGroup::UI) => vec![
                "Ensure prompt text is provided".to_string(),
                "Verify type is one of: text, select, multiselect".to_string(),
                "For select/multiselect, provide options array".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::UI) => vec![
                "Check template rendering parameters".to_string(),
                "Verify data format is valid for visualization".to_string(),
                "Try a simpler UI component".to_string(),
            ],

            // MCP Manager tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::McpManager) => vec![
                "Use listServers to see available MCP servers".to_string(),
                "Verify the server name is correct".to_string(),
                "Use searchServer to find servers by name".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::McpManager) => vec![
                "Ensure server name is provided".to_string(),
                "Verify transport configuration is valid".to_string(),
                "Check transport type is stdio or http".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::McpManager) => vec![
                "Check server configuration is correct".to_string(),
                "Verify the server binary/command exists".to_string(),
                "Use listServers to see server status".to_string(),
            ],

            // Playbook tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Playbook) => vec![
                "Use listPlaybooks to see available playbooks".to_string(),
                "Verify the playbook ID is correct".to_string(),
                "Use showPlaybooks for interactive selection".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Playbook) => vec![
                "Ensure goal and workflow are provided".to_string(),
                "Verify workflow is an array of steps".to_string(),
                "Check step structure includes required fields".to_string(),
            ],

            // Bootstrap tool errors
            (ErrorCategory::InvalidInput, ToolGroup::Bootstrap) => vec![
                "Verify tool parameter is provided".to_string(),
                "Tool must be one of: node, python, uv, docker, git".to_string(),
                "Platform must be: windows, linux, darwin, or auto".to_string(),
            ],

            // Generic fallbacks
            (ErrorCategory::MissingRequiredParam, _) => vec![
                "Check the tool documentation for required parameters".to_string(),
                "Ensure all required fields are provided".to_string(),
                "Review the example usage in the tool description".to_string(),
            ],
            (ErrorCategory::InternalError, _) => vec![
                "This is a system error - please try again".to_string(),
                "If the error persists, check system logs".to_string(),
                "Consider reporting this issue if it continues".to_string(),
            ],
            (ErrorCategory::DatabaseError, _) => vec![
                "This is a database error - please try again".to_string(),
                "Check if the database is accessible".to_string(),
                "Verify data integrity and try again".to_string(),
            ],

            // Default fallback for uncategorized combinations
            _ => vec![
                "Review the error message for specific details".to_string(),
                "Check tool documentation for correct usage".to_string(),
                "Try a simpler operation to isolate the issue".to_string(),
            ],
        }
    }
}

/// Success hint builder for tool chaining suggestions
pub struct SuccessHint {
    pub message: String,
    pub next_actions: Vec<String>,
}

impl SuccessHint {
    /// Create a success hint with suggested next actions
    pub fn new(message: impl Into<String>, next_actions: Vec<String>) -> Self {
        Self {
            message: message.into(),
            next_actions,
        }
    }

    /// Create a success MCPResult with hints
    pub fn to_mcp_result(&self) -> MCPResult {
        self.to_mcp_result_with_data(None)
    }

    /// Create a success MCPResult with hints and structured data
    pub fn to_mcp_result_with_data(&self, data: Option<serde_json::Value>) -> MCPResult {
        let hint_text = if self.next_actions.is_empty() {
            String::new()
        } else {
            format!("\n\n💡 Next: {}", self.next_actions.join(" or "))
        };

        let formatted_message = format!("✓ {}{}", self.message, hint_text);

        if let Some(data) = data {
            MCPResult::success_with_data(&formatted_message, data)
        } else {
            MCPResult::success(&formatted_message)
        }
    }

    /// Get suggested next actions for a tool within its group
    pub fn for_tool(tool_name: &str, tool_group: ToolGroup) -> Vec<String> {
        match (tool_name, tool_group) {
            // Browser tools
            ("createSession", ToolGroup::Browser) => vec![
                "Use navigateToUrl to load a webpage".to_string(),
                "Use extractWebContent to see the initial page".to_string(),
            ],
            ("navigateToUrl", ToolGroup::Browser) => vec![
                "Use extractWebContent to see page content".to_string(),
                "Use listInteractable to see clickable elements".to_string(),
            ],
            ("extractWebContent", ToolGroup::Browser) => vec![
                "Use listInteractable to see interactive elements".to_string(),
                "Use clickElement to interact with the page".to_string(),
            ],
            ("listInteractable", ToolGroup::Browser) => vec![
                "Use clickElement with the selector or index".to_string(),
                "Use extractWebContent to see full page content".to_string(),
            ],

            // Planning tools
            ("addTodo", ToolGroup::Planning) => vec![
                "Use getCurrentState to see all todos".to_string(),
                "Use checkTodo to modify details".to_string(),
                "Use checkTodo to mark as done".to_string(),
            ],
            ("createGoal", ToolGroup::Planning) => vec![
                "Use addTodo to create tasks for this goal".to_string(),
                "Use getCurrentState to see the full planning state".to_string(),
            ],
            ("checkTodo", ToolGroup::Planning) => vec![
                "Use getCurrentState to see remaining tasks".to_string(),
                "Use addTodo to create follow-up tasks".to_string(),
            ],
            ("getCurrentState", ToolGroup::Planning) => vec![
                "Use checkTodo to mark items as complete".to_string(),
                "Use addTodo to create new tasks".to_string(),
            ],

            // Workspace tools
            ("writeFile", ToolGroup::Workspace) => vec![
                "Use readFile to verify the content".to_string(),
                "Use listDirectory to see the file in context".to_string(),
            ],
            ("readFile", ToolGroup::Workspace) => vec![
                "Use writeFile to modify the content".to_string(),
                "Use replaceLines to make targeted edits".to_string(),
            ],
            ("listDirectory", ToolGroup::Workspace) => vec![
                "Use readFile to view file contents".to_string(),
                "Use writeFile to create new files".to_string(),
            ],
            ("runInPersistentShell", ToolGroup::Workspace) => vec![
                "Use listDirectory to verify created files".to_string(),
                "Use readFile to verify file contents".to_string(),
            ],

            // Assistant tools
            ("builtin_assistant__createAssistant", ToolGroup::Assistant) => vec![
                "Use builtin_assistant__listAssistants to see all assistants".to_string(),
                "Use builtin_assistant__updateAssistant to modify configuration".to_string(),
            ],
            ("builtin_assistant__updateAssistant", ToolGroup::Assistant) => {
                vec!["Use builtin_assistant__getAssistant to verify changes".to_string()]
            }

            // UI tools
            ("prompt_user", ToolGroup::UI) => {
                vec!["Use getUserAnswer to receive user response".to_string()]
            }
            ("visualize_data", ToolGroup::UI) => {
                vec!["Chart has been rendered and displayed to user".to_string()]
            }
            ("wait_for_user_resume", ToolGroup::UI) => {
                vec!["Use resumeFromWait when user clicks continue".to_string()]
            }

            // MCP Manager tools
            ("listServers", ToolGroup::McpManager) => vec![
                "Use connectServer to connect to a server".to_string(),
                "Use createServer to register new servers".to_string(),
            ],
            ("createServer", ToolGroup::McpManager) => {
                vec!["Use listServers to verify server was created".to_string()]
            }
            ("connectServer", ToolGroup::McpManager) => {
                vec!["Server is now available for tool calls".to_string()]
            }

            // Playbook tools
            ("createPlaybook", ToolGroup::Playbook) => vec![
                "Use selectPlaybook to execute this playbook".to_string(),
                "Use listPlaybooks to see all playbooks".to_string(),
            ],
            ("listPlaybooks", ToolGroup::Playbook) => vec![
                "Use selectPlaybook with ID to execute".to_string(),
                "Use showPlaybooks for interactive UI".to_string(),
            ],
            ("selectPlaybook", ToolGroup::Playbook) => {
                vec!["Review workflow steps and begin execution".to_string()]
            }

            // Bootstrap tools
            ("detectPlatform", ToolGroup::Bootstrap) => {
                vec!["Use getBootstrapGuide with detected platform".to_string()]
            }
            ("getBootstrapGuide", ToolGroup::Bootstrap) => {
                vec!["Follow installation steps for the tool".to_string()]
            }

            // Default: no specific hints
            _ => vec![],
        }
    }
}

/// Convenience functions for creating common errors with guidance
///
/// Create a "missing parameter" error with guidance
pub fn missing_param_error(param_name: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::MissingRequiredParam,
        format!("Missing required parameter: '{}'", param_name),
        tool_group,
    )
    .to_mcp_result()
}

/// Create a "resource not found" error with guidance
pub fn not_found_error(resource_type: &str, identifier: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::ResourceNotFound,
        format!("{} '{}' not found", resource_type, identifier),
        tool_group,
    )
    .to_mcp_result()
}

/// Create a "duplicate resource" error with guidance
pub fn duplicate_error(resource_type: &str, identifier: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::DuplicateResource,
        format!("{} '{}' already exists", resource_type, identifier),
        tool_group,
    )
    .to_mcp_result()
}

/// Create an "invalid input" error with guidance
pub fn invalid_input_error(message: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(ErrorCategory::InvalidInput, message, tool_group).to_mcp_result()
}

/// Create a "permission denied" error with guidance
pub fn permission_denied_error(resource: &str, tool_group: ToolGroup) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::PermissionDenied,
        format!("Permission denied: {}", resource),
        tool_group,
    )
    .to_mcp_result()
}

/// Create an "operation failed" error with custom guidance
pub fn operation_failed_error(
    operation: &str,
    reason: &str,
    guidance: Vec<String>,
    tool_group: ToolGroup,
) -> MCPResult {
    ErrorGuidance::with_guidance(
        ErrorCategory::OperationFailed,
        format!("{} failed: {}", operation, reason),
        guidance,
        tool_group,
    )
    .to_mcp_result()
}

#[cfg(test)]
mod tests {
    use super::*;

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
            if let Some(crate::mcp::types::MCPContent::Text { text }) = content.first() {
                assert!(text.contains("✗"));
                assert!(text.contains("💡 Next Steps:"));
                assert!(text.contains("Session 'abc123' not found"));
                assert!(text.contains("1. "));
            }
        }
    }

    #[test]
    fn test_success_hint_formatting() {
        let hint = SuccessHint::new(
            "Todo created successfully",
            vec![
                "Use getCurrentState to see all todos".to_string(),
                "Use checkTodo to modify".to_string(),
            ],
        );

        let result = hint.to_mcp_result();

        assert!(result.is_error == Some(false));

        if let Some(content) = result.content {
            if let Some(crate::mcp::types::MCPContent::Text { text }) = content.first() {
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
}
