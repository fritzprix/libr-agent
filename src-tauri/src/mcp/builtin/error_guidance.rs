/// Error Guidance System for Built-in MCP Tools
///
/// This module provides a centralized error guidance system that ensures consistent,
/// actionable error messages across all built-in tools. It follows the best practices
/// documented in docs/guides/builtin_tool_bp.md.
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
    Memory,
    Workspace,
    Assistant,
    ContentStore,
    Knowledge,
    Playbook,
    UI,
    McpManager,
    Bootstrap,
    Swarm,
}

/// Structured error with guidance
pub struct ErrorGuidance {
    pub category: ErrorCategory,
    pub message: String,
    pub guidance: Vec<String>,
    pub tool_group: ToolGroup,
}

/// Canonical builder for constructing `ErrorGuidance` instances.
///
/// Why this exists:
/// - Makes the "category + message + tool_group + guidance" contract explicit.
/// - Allows optional override of default guidance in a consistent way.
/// - Enables gradual migration without breaking the existing helper functions.
#[must_use]
pub struct ErrorBuilder {
    category: ErrorCategory,
    message: String,
    guidance: Option<Vec<String>>,
    tool_group: ToolGroup,
}

impl ErrorBuilder {
    /// Create a new builder.
    pub fn new(category: ErrorCategory, message: impl Into<String>, tool_group: ToolGroup) -> Self {
        Self {
            category,
            message: message.into(),
            guidance: None,
            tool_group,
        }
    }

    /// Override default guidance with custom recovery steps.
    pub fn guidance(mut self, guidance: Vec<String>) -> Self {
        self.guidance = Some(guidance);
        self
    }

    /// Alias for `guidance` to support builder pattern naming.
    pub fn with_guidance(self, guidance: Vec<String>) -> Self {
        self.guidance(guidance)
    }

    /// Build an `ErrorGuidance` instance.
    pub fn build(self) -> ErrorGuidance {
        let guidance = self
            .guidance
            .unwrap_or_else(|| ErrorGuidance::get_default_guidance(self.category, self.tool_group));

        ErrorGuidance {
            category: self.category,
            message: self.message,
            guidance,
            tool_group: self.tool_group,
        }
    }

    /// Convenience: Build and convert to `MCPResult`.
    pub fn to_mcp_result(self) -> MCPResult {
        self.build().to_mcp_result()
    }
}

/// Canonical entrypoint for creating guided errors.
///
/// Prefer this over calling `ErrorGuidance::new/with_guidance` directly in new code.
pub fn guided_error(
    category: ErrorCategory,
    message: impl Into<String>,
    tool_group: ToolGroup,
) -> ErrorBuilder {
    ErrorBuilder::new(category, message, tool_group)
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
                "Use updateTodo(action='done') to modify the existing item".to_string(),
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
                "Use listAssistants with 'search' parameter to find assistants by name".to_string(),
            ],
            (ErrorCategory::DuplicateResource, ToolGroup::Assistant) => vec![
                "Use a different name for the new assistant".to_string(),
                "Use updateAssistant to modify the existing one".to_string(),
                "Use listAssistants to see all assistants".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Assistant) => vec![
                "Verify all required parameters are provided".to_string(),
                "Check that parameter values are in the correct format".to_string(),
                "Use listAssistants or getAssistant to verify existing data".to_string(),
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
                "Use searchContent to find content by keywords".to_string(),
            ],

            // Knowledge tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Knowledge) => vec![
                "Use listKnowledge to see available knowledge entries".to_string(),
                "Use searchContent to find entries by keyword".to_string(),
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
                "Use listTools to see available MCP servers".to_string(),
                "Verify the server name is correct".to_string(),
                "Use listTools with a query to search servers by name".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::McpManager) => vec![
                "Ensure server name is provided".to_string(),
                "Verify transport configuration is valid".to_string(),
                "Check transport type is stdio or http".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::McpManager) => vec![
                "Check server configuration is correct".to_string(),
                "Verify the server binary/command exists".to_string(),
                "Use listTools to see server status".to_string(),
            ],

            // Playbook tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Playbook) => vec![
                "Use listPlaybooks to see available playbooks".to_string(),
                "Verify the playbook ID is correct".to_string(),
                "Use getPlaybookPage for interactive selection".to_string(),
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

            // Swarm tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Swarm) => vec![
                "Verify the ID (agent ID or assistant ID) is correct".to_string(),
                "Use listAssistants to find available assistant configurations".to_string(),
                "Use getChildAgents to list active sub-agents or sessions".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Swarm) => vec![
                "Check all required parameters are provided and correctly typed".to_string(),
                "Review the tool schema for required fields and formats".to_string(),
                "Check agent status with getAgentStatus if the session already exists".to_string(),
            ],
            (ErrorCategory::NetworkError, ToolGroup::Swarm) => vec![
                "The internal swarm service may not be running — check the application state"
                    .to_string(),
                "Restart the application if this error persists".to_string(),
                "Verify session connectivity with getAgentStatus".to_string(),
            ],
            (ErrorCategory::Timeout, ToolGroup::Swarm) => vec![
                "Increase the timeoutSeconds parameter for slow tasks".to_string(),
                "Use awaitAgent with a longer timeout".to_string(),
                "Check agent status with getAgentStatus".to_string(),
            ],
            (ErrorCategory::PermissionDenied, ToolGroup::Swarm) => vec![
                "Check agent nesting depth constraints (maxDepth)".to_string(),
                "Verify you have permission to spawn agents in this context".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::Swarm) => vec![
                "Retry the operation".to_string(),
                "Use getChildAgents to see current agent state".to_string(),
                "Check agent status with getAgentStatus".to_string(),
            ],
            (ErrorCategory::InternalError, ToolGroup::Swarm) => vec![
                "Retry the operation — this is likely a transient error".to_string(),
                "Check application logs for details".to_string(),
            ],

            // Memory tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Memory) => vec![
                "Use memory__list to see available notes".to_string(),
                "Verify the ID is correct".to_string(),
            ],
            (ErrorCategory::DuplicateResource, ToolGroup::Memory) => vec![
                "Use a different title for the new note".to_string(),
                "Use memory__update to modify the existing note".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Memory) => vec![
                "Ensure all required parameters are provided".to_string(),
                "Use memory__list to see current notes for reference".to_string(),
            ],
            (ErrorCategory::InvalidState, ToolGroup::Memory) => vec![
                "Use memory__clear to remove old items".to_string(),
                "Use memory__update to modify existing notes".to_string(),
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
                "Use updateTodo(index=N, action='done') to mark as complete".to_string(),
            ],
            ("createGoal", ToolGroup::Planning) => vec![
                "Use addTodo to create tasks for this goal".to_string(),
                "Use getCurrentState to see the full planning state".to_string(),
            ],
            ("updateTodo", ToolGroup::Planning) => vec![
                "Use getCurrentState to see remaining tasks".to_string(),
                "Use addTodo to create follow-up tasks".to_string(),
                "When all todos are done, use reflect to review progress".to_string(),
            ],
            ("getCurrentState", ToolGroup::Planning) => vec![
                "Use updateTodo(index=N, action='done') to mark items as complete".to_string(),
                "Use addTodo to create new tasks".to_string(),
            ],
            ("reflect", ToolGroup::Planning) => vec![
                "Proceed with the next action identified in your reflection".to_string(),
                "Use createGoal if starting a new task after reflection".to_string(),
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
            ("assistant__createAssistant", ToolGroup::Assistant) => vec![
                "Use assistant__listAssistants to see all assistants".to_string(),
                "Use assistant__updateAssistant to modify configuration".to_string(),
            ],
            ("assistant__updateAssistant", ToolGroup::Assistant) => {
                vec!["Use assistant__getAssistant to verify changes".to_string()]
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
            ("listTools", ToolGroup::McpManager) => vec![
                "Use registerServer to add new external servers".to_string(),
                "Use updateAssistant to give an assistant access to found servers".to_string(),
            ],
            ("registerServer", ToolGroup::McpManager) => {
                vec!["Use listTools to verify server was created".to_string()]
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
                "Use getPlaybookPage for interactive UI".to_string(),
            ],
            ("selectPlaybook", ToolGroup::Playbook) => {
                vec!["Review workflow steps and begin execution".to_string()]
            }

            // Swarm tools
            ("spawnAgent", ToolGroup::Swarm) => vec![
                "Use awaitAgent with the session ID to wait for completion".to_string(),
                "Use getChildAgents to see all active sub-agents".to_string(),
                "Use listMessages to see progress if not awaiting".to_string(),
            ],

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
            if let Some(crate::mcp::types::MCPContent::Text { text, .. }) = content.first() {
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
                "Use updateTodo(index=N, action='done') to mark as complete".to_string(),
            ],
        );

        let result = hint.to_mcp_result();

        assert!(result.is_error == Some(false));

        if let Some(content) = result.content {
            if let Some(crate::mcp::types::MCPContent::Text { text, .. }) = content.first() {
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
        let crate::mcp::types::MCPContent::Text { text, .. } = content
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
    fn test_guided_error_builder_allows_override_guidance() {
        let result = guided_error(ErrorCategory::InvalidInput, "Bad input", ToolGroup::UI)
            .guidance(vec!["Use prompt_user with type='text'".to_string()])
            .to_mcp_result();

        let content = result.content.expect("Expected MCPResult.content");
        let crate::mcp::types::MCPContent::Text { text, .. } = content
            .first()
            .expect("Expected at least one content item")
            .clone()
        else {
            panic!("Expected Text content");
        };

        // Should contain the override guidance, and should not need to match UI defaults.
        assert!(text.contains("1. Use prompt_user with type='text'"));
    }
}
