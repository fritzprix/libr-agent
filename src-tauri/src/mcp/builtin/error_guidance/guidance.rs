use super::categories::{ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;

/// Shared section headers for agent-facing hints (success, failure recovery, static tool docs).
pub mod hint_headers {
    pub const SUCCESS_FOLLOW_UPS: &str = "💡 Suggested Follow-ups:";
    pub const ERROR_RECOVERY: &str = "💡 Suggested Recovery:";
    pub const NOTICE_GUIDANCE: &str = "💡 Optional Guidance:";
    pub const TOOL_RELATED_ACTIONS: &str = "💡 Related Actions:";
    pub const TOOL_EXAMPLE_WORKFLOW: &str = "💡 Example workflow:";
    pub const TIP: &str = "💡 Tip:";
    pub const AVAILABLE_OPERATIONS: &str = "💡 Available operations:";
}

fn format_numbered_guidance(steps: &[String]) -> String {
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| format!("{}. {}", index + 1, step))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_bullet_guidance(lines: &[String]) -> String {
    lines
        .iter()
        .map(|line| format!("• {}", line))
        .collect::<Vec<_>>()
        .join("\n")
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
        let guidance_text = format_numbered_guidance(&self.guidance);

        let formatted_message = if self.category.uses_error_semantics() {
            format!(
                "✗ {}\n\n{}\n{}",
                self.message,
                hint_headers::ERROR_RECOVERY,
                guidance_text
            )
        } else {
            format!(
                "Notice: {}\n\n{}\n{}",
                self.message,
                hint_headers::NOTICE_GUIDANCE,
                guidance_text
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
                "Use browser__createSession to start a new browser session".to_string(),
                "Use browser__getPageContent({}) to extract fresh content from the current page".to_string(),
                "Use browser__listInteractable to inspect the current page before interacting".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Browser) => vec![
                "Verify the URL format is valid (must include http:// or https://)".to_string(),
                "Check selector syntax matches CSS selector standards".to_string(),
                "Use browser__listInteractable to see available elements first".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::Browser) => vec![
                "Try browser__getPageContent to view page structure".to_string(),
                "Use browser__navigateToUrl to reload the page".to_string(),
                "Verify the target element is visible and interactable".to_string(),
            ],

            // Planning tool errors
            (ErrorCategory::DuplicateResource, ToolGroup::Planning) => vec![
                "Use a different title for the new item".to_string(),
                "Use planning__updateTodo(id=..., action='done') to modify the existing item".to_string(),
                "Use planning__getCurrentState to see all existing items".to_string(),
            ],
            (ErrorCategory::ResourceNotFound, ToolGroup::Planning) => vec![
                "Use planning__getCurrentState to see available todos".to_string(),
                "Verify the ID is correct and the item exists".to_string(),
                "Use planning__getCurrentState to see the full planning state".to_string(),
                "Create as top-level todo instead".to_string(),
                "Attach to a different parent that has no parent".to_string(),
                "Use planning__getCurrentState to see the current hierarchy".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Planning) => vec![
                "Ensure title is a non-empty string".to_string(),
                "Priority must be 'low', 'medium', or 'high'".to_string(),
                "Use planning__getCurrentState to see existing todos for reference".to_string(),
            ],

            // Workspace tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Workspace) => vec![
                "Use workspace__listDirectory to see available files".to_string(),
                "Verify the file path is correct".to_string(),
                "Check if the file exists in the expected location".to_string(),
            ],
            (ErrorCategory::PermissionDenied, ToolGroup::Workspace) => vec![
                "Check file permissions with workspace__listDirectory".to_string(),
                "Ensure the path is within allowed directories".to_string(),
                "Verify you have read/write access to the target".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Workspace) => vec![
                "Verify the file path format is correct".to_string(),
                "Check that all required parameters are provided".to_string(),
                "Use workspace__listDirectory to see the correct path structure".to_string(),
            ],

            // Agent tool errors (Unified Assistant/Swarm)
            (ErrorCategory::ResourceNotFound, ToolGroup::Agent) => vec![
                "Verify the agentId or sessionId is correct".to_string(),
                "Use agent__listAgents(type=\"configs\") to find available agent configurations"
                    .to_string(),
                "Use agent__listAgents(type=\"sessions\") or agent__checkSession to inspect active delegated sessions"
                    .to_string(),
            ],
            (ErrorCategory::DuplicateResource, ToolGroup::Agent) => vec![
                "Use a different name for the new Agent or Assistant".to_string(),
                "Use agent__updateAgent to modify existing configurations".to_string(),
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
                "Use agent__checkSession with a longer timeout for async operations".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::Agent) => vec![
                "Retry the operation — it may be a transient failure".to_string(),
                "Check Agent status with agent__checkSession for specific errors".to_string(),
            ],

            // Attachments tool errors
            (ErrorCategory::InvalidFormat, ToolGroup::Attachments) => vec![
                "Ensure the file format is supported (PDF, HTML, markdown, code)".to_string(),
                "Check the file is not corrupted".to_string(),
                "Try a different file or format".to_string(),
            ],
            (ErrorCategory::ResourceNotFound, ToolGroup::Attachments) => vec![
                "Use attachments__listAttachments to see available attachments".to_string(),
                "Verify the content ID is correct".to_string(),
                "Use attachments__searchAttachments to find attachments by keywords".to_string(),
            ],

            // Knowledge tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Knowledge) => vec![
                "Use knowledge__searchKnowledge to find relevant knowledge chunks and their IDs".to_string(),
                "Use knowledge__exploreContext when you need graph context around a known entity".to_string(),
                "Retry the request with IDs copied from prior knowledge results".to_string(),
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
                "Use scheduled_task__listScheduledTasks() to see available scheduled task IDs".to_string(),
                "Retry with an exact task ID copied from scheduled_task__listScheduledTasks()".to_string(),
                "Use scheduled_task__getScheduledTask(id) to inspect a task before mutating it".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::ScheduledTask) => vec![
                "Verify cronExpression, assistantId, and workspaceOverride values".to_string(),
                "Use an absolute directory path for workspaceOverride".to_string(),
                "Use scheduleTimezone of 'local' or 'utc'".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::ScheduledTask) => vec![
                "Use scheduled_task__listScheduledTasks() to inspect the current schedule state".to_string(),
                "Use scheduled_task__getScheduledTask(id) to verify the target task before retrying".to_string(),
                "Check whether the target assistant still exists".to_string(),
            ],

            // MCP Manager tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Tool) => vec![
                "Use tool__listServers({\"availability\":\"inventory\"}) to see available MCP servers"
                    .to_string(),
                "Verify the server name is correct".to_string(),
                "Use tool__listServers({\"availability\":\"inventory\",\"query\":\"<name>\"}) to search servers by name".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Tool) => vec![
                "Ensure server name is provided".to_string(),
                "Verify transport configuration is valid".to_string(),
                "Check transport type is stdio or http".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::Tool) => vec![
                "Check server configuration is correct".to_string(),
                "Verify the server binary/command exists".to_string(),
                "Use tool__listServers({\"availability\":\"inventory\"}) to see server status"
                    .to_string(),
            ],

            // Playbook tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Playbook) => vec![
                "Use playbook__listPlaybooks to see available playbooks".to_string(),
                "Verify the playbook ID is correct".to_string(),
                "Use playbook__getPlaybookPage for interactive selection".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Playbook) => vec![
                "Ensure goal and workflow are provided".to_string(),
                "Verify workflow is an array of steps".to_string(),
                "Check step structure includes required fields".to_string(),
            ],

            // Setup Wizard tool errors
            (ErrorCategory::InvalidInput, ToolGroup::SetupWizard) => vec![
                "Verify tool parameter is provided".to_string(),
                "Tool must be one of: node, python, uv, docker, git".to_string(),
                "Platform must be: windows, linux, darwin, or auto".to_string(),
            ],

            // Scratchpad tool errors
            (ErrorCategory::ResourceNotFound, ToolGroup::Scratchpad) => vec![
                "Use scratchpad__listNote to see available notes".to_string(),
                "Verify the ID is correct".to_string(),
            ],
            (ErrorCategory::DuplicateResource, ToolGroup::Scratchpad) => vec![
                "Use a different title for the new note".to_string(),
                "Use scratchpad__updateNote to modify the existing note".to_string(),
            ],
            (ErrorCategory::InvalidInput, ToolGroup::Scratchpad) => vec![
                "Ensure all required parameters are provided".to_string(),
                "Use scratchpad__listNote to see current notes for reference".to_string(),
            ],
            (ErrorCategory::InvalidState, ToolGroup::Scratchpad) => vec![
                "Use scratchpad__clearNote to remove old items".to_string(),
                "Use scratchpad__updateNote to modify existing notes".to_string(),
            ],

            // Media tool errors
            (ErrorCategory::InvalidInput, ToolGroup::Media) => vec![
                "Provide a valid URL (https://...) or a workspace-relative path".to_string(),
                "Supported image formats: JPEG, PNG, GIF, WebP, BMP, SVG".to_string(),
                "Supported audio formats: MP3, WAV, OGG, AAC, FLAC, WEBM".to_string(),
            ],
            (ErrorCategory::OperationFailed, ToolGroup::Media) => vec![
                "Verify the URL is publicly accessible and returns a 200 response".to_string(),
                "Check that the file size is under 20 MB".to_string(),
                "For local files, use a workspace-relative path instead".to_string(),
            ],
            (ErrorCategory::PermissionDenied, ToolGroup::Media) => vec![
                "The file path must be inside the session workspace".to_string(),
                "Use a relative path such as 'screenshots/image.png'".to_string(),
                "Use workspace__listDirectory() to confirm the file location within the workspace".to_string(),
            ],
            (ErrorCategory::ResourceNotFound, ToolGroup::Media) => vec![
                "Use workspace__listDirectory() to verify the file exists in the workspace".to_string(),
                "Check for typos in the path".to_string(),
                "Use a workspace-relative path (e.g. 'images/photo.png')".to_string(),
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
            format!(
                "\n\n{}\n{}",
                hint_headers::SUCCESS_FOLLOW_UPS,
                format_bullet_guidance(&self.next_actions)
            )
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
                "Use browser__navigateToUrl to load a webpage".to_string(),
                "Use browser__getPageContent to see the initial page".to_string(),
            ],
            ("navigateToUrl", ToolGroup::Browser) => vec![
                "Use browser__getPageContent to see page content".to_string(),
                "Use browser__listInteractable to see clickable elements".to_string(),
            ],
            ("getPageContent", ToolGroup::Browser) => vec![
                "Use browser__listInteractable to see interactive elements".to_string(),
                "Use browser__clickElement to interact with the page".to_string(),
            ],
            ("listInteractable", ToolGroup::Browser) => vec![
                "Use browser__clickElement with the selector".to_string(),
                "Use browser__getPageContent to see full page content".to_string(),
            ],

            // Planning tools
            ("addTodo", ToolGroup::Planning) => vec![
                "Use planning__getCurrentState to see all todos".to_string(),
                "Use planning__updateTodo(id=..., action='done') to mark as complete".to_string(),
            ],
            ("createGoal", ToolGroup::Planning) => vec![
                "Use planning__addTodo to create tasks for this goal".to_string(),
                "Use planning__getCurrentState to see the full planning state".to_string(),
            ],
            ("updateTodo", ToolGroup::Planning) => vec![
                "Use planning__getCurrentState to see remaining tasks".to_string(),
                "Use planning__addTodo to create follow-up tasks".to_string(),
                "When all todos are done, use planning__reflect to review progress".to_string(),
            ],
            ("getCurrentState", ToolGroup::Planning) => vec![
                "Use planning__updateTodo(id=..., action='done') to mark items as complete".to_string(),
                "Use planning__addTodo to create new tasks".to_string(),
            ],
            ("reflect", ToolGroup::Planning) => vec![
                "Proceed with the next action identified in your reflection".to_string(),
                "Use planning__createGoal if starting a new task after reflection".to_string(),
            ],

            // Workspace tools
            ("runShell" | "runPowerShell", ToolGroup::Workspace) => vec![],
            ("runInPersistentShell" | "runInPersistentPowerShell", ToolGroup::Workspace) => vec![
                "Command state (CWD, env vars) is preserved for the next call".to_string(),
                "With requireUserInput=true, the same synchronous call can pause for a human prompt and then resume to a final result".to_string(),
                "Use shell commands like `pwd` or `ls` in the next persistent-shell call to inspect the current shell directory".to_string(),
                "workspace__readFile and workspace__listDirectory still use workspace root, not the shell CWD".to_string(),
            ],
            ("spawnProcess", ToolGroup::Workspace) => vec![
                "Use workspace__listProcesses to see the status of the background task".to_string(),
                "Use workspace__waitForProcess with timeout=0 to check if it's still running".to_string(),
                "Use workspace__readProcessOutput to see standard output and error".to_string(),
            ],
            ("waitForProcess" | "pollProcess", ToolGroup::Workspace) => vec![
                "Use workspace__readProcessOutput to see the final results".to_string(),
            ],
            ("readProcessOutput", ToolGroup::Workspace) => vec![
                "Works while the process is still running; finishing is not required".to_string(),
                "Use workspace__waitForProcess(processId, 0) only to check status, not to unlock reading".to_string(),
                "If processId is missing from the registry, use workspace__listProcesses() — do not assume it is still starting".to_string(),
            ],
            ("listProcesses", ToolGroup::Workspace) => vec![],
            ("writeFile", ToolGroup::Workspace) => vec![
                "Use workspace__readFile to verify the content".to_string(),
                "Use workspace__listDirectory to see the file in context".to_string(),
            ],
            ("readFile", ToolGroup::Workspace) => vec![
                format!(
                    "Use workspace__{} for targeted in-place edits",
                    crate::mcp::builtin::workspace::edit_mode::PRIMARY_EDIT_TOOL
                ),
                "Use workspace__writeFile only to create, overwrite, or append whole files".to_string(),
            ],
            ("listDirectory", ToolGroup::Workspace) => vec![
                "Use workspace__readFile to view file contents".to_string(),
                "Use workspace__writeFile to create new files".to_string(),
                "Use workspace__globFiles with filePattern to narrow down names".to_string(),
            ],
            #[cfg(feature = "workspace-edit-file")]
            (
                "editFile" | "replaceLines" | "insertAfterLine" | "deleteLines",
                ToolGroup::Workspace,
            ) => vec![
                "Use workspace__readFile to verify your edits".to_string(),
                "Use workspace__runShell to execute the updated code".to_string(),
            ],
            #[cfg(feature = "workspace-str-replace")]
            ("strReplace", ToolGroup::Workspace) => vec![
                "Use workspace__readFile to verify your edits".to_string(),
                "Use workspace__runShell to execute the updated code".to_string(),
            ],
            ("globFiles", ToolGroup::Workspace) => vec![
                "Use workspace__grepFiles to search inside matched files".to_string(),
                "Use workspace__readFile to inspect a specific match".to_string(),
            ],
            ("grepFiles", ToolGroup::Workspace) => vec![
                "Use workspace__readFile on interesting matches".to_string(),
                format!(
                    "Use workspace__{} to make targeted edits",
                    crate::mcp::builtin::workspace::edit_mode::PRIMARY_EDIT_TOOL
                ),
            ],
            ("searchFiles", ToolGroup::Workspace) => vec![
                "Use workspace__globFiles for filename search".to_string(),
                "Use workspace__grepFiles for content search".to_string(),
            ],

            // Agent tools (Unified)
            ("createAgent", ToolGroup::Agent) => vec![
                "Use agent__listAgents to see all agents".to_string(),
                "Use agent__updateAgent to modify configuration".to_string(),
                "Use agent__startSession to begin work with this agent".to_string(),
            ],
            ("updateAgent", ToolGroup::Agent) => vec![
                "Use agent__listAgents to verify the configuration updates".to_string(),
                "Use agent__startSession to apply changes in a new session".to_string(),
            ],
            ("startSession", ToolGroup::Agent) => vec![
                "Use agent__messageToSession only for follow-up instructions after the session starts"
                    .to_string(),
                "Use agent__checkSession to see if work is complete".to_string(),
            ],
            ("messageToSession" | "checkSession", ToolGroup::Agent) => vec![
                "Continue monitoring progress with agent__checkSession".to_string(),
                "Use agent__stopSession when the task is fully accomplished".to_string(),
            ],
            // Legacy alias key kept for callers that still pass obsolete spawnAgent name
            ("spawnAgent", ToolGroup::Agent) => vec![
                "Use agent__checkSession with the session ID to wait for completion".to_string(),
                "Use agent__listAgents(type=\"sessions\") to see active delegated sessions"
                    .to_string(),
            ],
            ("createScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use scheduled_task__getScheduledTask(id) to inspect the created schedule".to_string(),
                "Use scheduled_task__listScheduledTasks() to coordinate related recurring tasks".to_string(),
            ],
            ("listScheduledTasks", ToolGroup::ScheduledTask) => vec![
                "Use scheduled_task__getScheduledTask(id) to inspect one task in detail".to_string(),
                "Use scheduled_task__updateScheduledTask(id, ...) to revise a selected schedule".to_string(),
            ],
            ("getScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use scheduled_task__updateScheduledTask(id, ...) to revise the schedule".to_string(),
                "Use scheduled_task__toggleScheduledTask(id, enabled=false) to pause it without deleting"
                    .to_string(),
            ],
            ("updateScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use scheduled_task__getScheduledTask(id) to confirm the persisted next run".to_string(),
                "Use scheduled_task__toggleScheduledTask(id, enabled=false) if the updated schedule should pause"
                    .to_string(),
            ],
            ("toggleScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use scheduled_task__getScheduledTask(id) to confirm the new enabled state".to_string(),
                "Use scheduled_task__updateScheduledTask(id, ...) if the schedule itself must change".to_string(),
            ],
            ("deleteScheduledTask", ToolGroup::ScheduledTask) => vec![
                "Use scheduled_task__listScheduledTasks() to verify the remaining schedule set".to_string(),
                "Create a replacement with scheduled_task__createScheduledTask(...) if removal was intentional"
                    .to_string(),
            ],

            // Tool Management (Unified)
            ("listServers", ToolGroup::Tool) => vec![
                "Use tool__registerServer to add new external servers".to_string(),
                "Use tool__updateServer to refresh server configuration".to_string(),
            ],
            ("registerServer", ToolGroup::Tool) => vec![
                "Use tool__listServers to verify server was created".to_string(),
                "Use tool__verifyServer to check server health".to_string(),
            ],
            ("verifyServer", ToolGroup::Tool) => {
                vec!["Server is ready for tool calls if verification passed".to_string()]
            }
            ("connectServer", ToolGroup::Tool) => {
                vec!["Server tools are now available for use".to_string()]
            }

            // Setup Wizard tools
            ("detectPlatform", ToolGroup::SetupWizard) => {
                vec!["Use setup-wizard__getSetupGuide with detected platform".to_string()]
            }
            ("getSetupGuide", ToolGroup::SetupWizard) => {
                vec!["Follow installation steps for the tool".to_string()]
            }

            // Default: no specific hints
            _ => vec![],
        }
    }
}
