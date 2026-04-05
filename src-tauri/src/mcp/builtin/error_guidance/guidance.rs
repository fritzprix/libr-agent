use super::categories::{ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;

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

        let formatted_message = if self.category.uses_error_semantics() {
            format!("✗ {}\n\n💡 Next Steps:\n{}", self.message, guidance_text)
        } else {
            format!(
                "Notice: {}\n\n💡 Next Steps:\n{}",
                self.message, guidance_text
            )
        };

        if self.category.uses_error_semantics() {
            MCPResult::error(&formatted_message)
        } else {
            MCPResult::informational(&formatted_message)
        }
    }

    /// Get default guidance for an error category within a tool group
    pub(crate) fn get_default_guidance(
        category: ErrorCategory,
        tool_group: ToolGroup,
    ) -> Vec<String> {
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
                "Use updateTodo(todoId=..., action='done') to modify the existing item".to_string(),
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

            // Agent tool errors (Unified Assistant/Swarm)
            (ErrorCategory::ResourceNotFound, ToolGroup::Agent) => vec![
                "Verify the agentId or sessionId is correct".to_string(),
                "Use list(type=\"configs\") to find available agent configurations".to_string(),
                "Use list(type=\"sessions\") or checkSession to inspect active delegated sessions"
                    .to_string(),
            ],
            (ErrorCategory::DuplicateResource, ToolGroup::Agent) => vec![
                "Use a different name for the new Agent or Assistant".to_string(),
                "Use agent__update to modify existing configurations".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Agent) => vec![
                "Verify all required parameters (goal, inputs) are provided".to_string(),
                "Check the tool schema for required fields and formats".to_string(),
                "Review the system prompt and role configuration".to_string(),
            ],
            (ErrorCategory::NetworkError, ToolGroup::Agent) => vec![
                "The internal Agent service may not be running".to_string(),
                "Restart the application if connectivity issues persist".to_string(),
            ],
            (ErrorCategory::Timeout, ToolGroup::Agent) => vec![
                "Increase the timeoutSeconds parameter for complex tasks".to_string(),
                "Use awaitAgent with a longer timeout for async operations".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::Agent) => vec![
                "Retry the operation — it may be a transient failure".to_string(),
                "Check Agent status with getAgentStatus for specific errors".to_string(),
            ],

            // Attachments tool errors
            (ErrorCategory::InvalidFormat, ToolGroup::Attachments) => vec![
                "Ensure the file format is supported (PDF, HTML, markdown, code)".to_string(),
                "Check the file is not corrupted".to_string(),
                "Try a different file or format".to_string(),
            ],
            (ErrorCategory::ResourceNotFound, ToolGroup::Attachments) => vec![
                "Use list to see available attachments".to_string(),
                "Verify the content ID is correct".to_string(),
                "Use search to find attachments by keywords".to_string(),
            ],

            // Knowledge tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Knowledge) => vec![
                "Use listKnowledge to see available knowledge entries".to_string(),
                "Use searchKnowledge to find entries by keyword".to_string(),
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
            (ErrorCategory::ResourceNotFound, ToolGroup::ScheduledTask) => vec![
                "Use listScheduledTasks() to see available scheduled task IDs".to_string(),
                "Retry with an exact task ID copied from listScheduledTasks()".to_string(),
                "Use getScheduledTask(id) to inspect a task before mutating it".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::ScheduledTask) => vec![
                "Verify cronExpression, assistantId, and workspaceOverride values".to_string(),
                "Use an absolute directory path for workspaceOverride".to_string(),
                "Use scheduleTimezone of 'local' or 'utc'".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::ScheduledTask) => vec![
                "Use listScheduledTasks() to inspect the current schedule state".to_string(),
                "Use getScheduledTask(id) to verify the target task before retrying".to_string(),
                "Check whether the target assistant still exists".to_string(),
            ],

            // MCP Manager tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Tool) => vec![
                "Use listTools to see available MCP servers".to_string(),
                "Verify the server name is correct".to_string(),
                "Use listTools with a query to search servers by name".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Tool) => vec![
                "Ensure server name is provided".to_string(),
                "Verify transport configuration is valid".to_string(),
                "Check transport type is stdio or http".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::Tool) => vec![
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

            // Scratchpad tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Scratchpad) => vec![
                "Use scratchpad__list to see available notes".to_string(),
                "Verify the ID is correct".to_string(),
            ],
            (ErrorCategory::DuplicateResource, ToolGroup::Scratchpad) => vec![
                "Use a different title for the new note".to_string(),
                "Use scratchpad__update to modify the existing note".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Scratchpad) => vec![
                "Ensure all required parameters are provided".to_string(),
                "Use scratchpad__list to see current notes for reference".to_string(),
            ],
            (ErrorCategory::InvalidState, ToolGroup::Scratchpad) => vec![
                "Use scratchpad__clear to remove old items".to_string(),
                "Use scratchpad__update to modify existing notes".to_string(),
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
                "Use updateTodo(todoId=..., action='done') to mark as complete".to_string(),
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
                "Use updateTodo(todoId=..., action='done') to mark items as complete".to_string(),
                "Use addTodo to create new tasks".to_string(),
            ],
            ("reflect", ToolGroup::Planning) => vec![
                "Proceed with the next action identified in your reflection".to_string(),
                "Use createGoal if starting a new task after reflection".to_string(),
            ],

            // Workspace tools
            ("runShell" | "runPowerShell", ToolGroup::Workspace) => vec![
                "Use listDirectory to verify created files".to_string(),
                "Use readFile to verify file contents".to_string(),
                "Use stopProcess if a long-running command needs to be terminated".to_string(),
            ],
            ("runInPersistentShell" | "runInPersistentPowerShell", ToolGroup::Workspace) => vec![
                "Command state (CWD, env vars) is preserved for the next call".to_string(),
                "Use listDirectory to verify file system changes".to_string(),
                "Use readFile to check written output".to_string(),
            ],
            ("spawnProcess", ToolGroup::Workspace) => vec![
                "Use listProcesses to see the status of the background task".to_string(),
                "Use waitForProcess with timeout=0 to check if it's still running".to_string(),
                "Use readProcessOutput to see standard output and error".to_string(),
            ],
            ("waitForProcess" | "pollProcess", ToolGroup::Workspace) => vec![
                "Use readProcessOutput to see the final results".to_string(),
                "Use listDirectory to check for any generated artifacts".to_string(),
            ],
            ("readProcessOutput", ToolGroup::Workspace) => vec![
                "Analyze the output to verify command success".to_string(),
                "Use stopProcess if the output indicates it's stuck".to_string(),
            ],
            ("listProcesses", ToolGroup::Workspace) => vec![
                "Use waitForProcess(processId) to block until completion".to_string(),
                "Use stopProcess(processId) to kill a stuck task".to_string(),
            ],
            ("writeFile", ToolGroup::Workspace) => vec![
                "Use readFile to verify the content".to_string(),
                "Use listDirectory to see the file in context".to_string(),
            ],
            ("readFile", ToolGroup::Workspace) => vec![
                "Use writeFile to modify the content".to_string(),
                "Use replaceLines, insertAfterLine, or deleteLines to make targeted edits"
                    .to_string(),
            ],
            ("listDirectory", ToolGroup::Workspace) => vec![
                "Use readFile to view file contents".to_string(),
                "Use writeFile to create new files".to_string(),
            ],
            (
                "editFile" | "replaceLines" | "insertAfterLine" | "deleteLines",
                ToolGroup::Workspace,
            ) => vec![
                "Use readFile to verify your edits".to_string(),
                "Use runShell to execute the updated code".to_string(),
            ],
            ("search" | "searchFiles", ToolGroup::Workspace) => vec![
                "Use readFile on interesting matches".to_string(),
                "Use listDirectory to explore the surrounding module".to_string(),
            ],

            // Agent tools (Unified)
            ("create", ToolGroup::Agent) => vec![
                "Use list to see all agents".to_string(),
                "Use update to modify configuration".to_string(),
                "Use startSession to begin work with this agent".to_string(),
            ],
            ("update", ToolGroup::Agent) => vec![
                "Use list to verify the configuration updates".to_string(),
                "Use startSession to apply changes in a new session".to_string(),
            ],
            ("startSession", ToolGroup::Agent) => vec![
                "Use messageToSession only for follow-up instructions after the session starts"
                    .to_string(),
                "Use checkSession to see if work is complete".to_string(),
            ],
            ("messageToSession" | "checkSession", ToolGroup::Agent) => vec![
                "Continue monitoring progress with checkSession".to_string(),
                "Use stopSession when the task is fully accomplished".to_string(),
            ],
            ("spawnAgent", ToolGroup::Agent) => vec![
                "Use awaitAgent with the session ID to wait for completion".to_string(),
                "Use getChildAgents to see all active sub-agents".to_string(),
            ],
            ("createScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use getScheduledTask(id) to inspect the created schedule".to_string(),
                "Use listScheduledTasks() to coordinate related recurring tasks".to_string(),
            ],
            ("listScheduledTasks", ToolGroup::ScheduledTask) => vec![
                "Use getScheduledTask(id) to inspect one task in detail".to_string(),
                "Use updateScheduledTask(id, ...) to revise a selected schedule".to_string(),
            ],
            ("getScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use updateScheduledTask(id, ...) to revise the schedule".to_string(),
                "Use toggleScheduledTask(id, enabled=false) to pause it without deleting"
                    .to_string(),
            ],
            ("updateScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use getScheduledTask(id) to confirm the persisted next run".to_string(),
                "Use toggleScheduledTask(id, enabled=false) if the updated schedule should pause"
                    .to_string(),
            ],
            ("toggleScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use getScheduledTask(id) to confirm the new enabled state".to_string(),
                "Use updateScheduledTask(id, ...) if the schedule itself must change".to_string(),
            ],
            ("deleteScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use listScheduledTasks() to verify the remaining schedule set".to_string(),
                "Create a replacement with createScheduledTask(...) if removal was intentional"
                    .to_string(),
            ],

            // Tool Management (Unified)
            ("list", ToolGroup::Tool) => vec![
                "Use register to add new external servers".to_string(),
                "Use update to refresh server configuration".to_string(),
            ],
            ("register", ToolGroup::Tool) => vec![
                "Use list to verify server was created".to_string(),
                "Use verify to check server health".to_string(),
            ],
            ("verify", ToolGroup::Tool) => {
                vec!["Server is ready for tool calls if verification passed".to_string()]
            }
            ("connectServer", ToolGroup::Tool) => {
                vec!["Server tools are now available for use".to_string()]
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
