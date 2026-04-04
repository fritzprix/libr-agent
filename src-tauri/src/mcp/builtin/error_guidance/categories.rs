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

impl ErrorCategory {
    /// Returns true when the tool completed unsuccessfully and should surface
    /// MCP `isError: true` semantics to the agent.
    pub fn uses_error_semantics(self) -> bool {
        matches!(
            self,
            Self::MissingRequiredParam
                | Self::InvalidInput
                | Self::InvalidFormat
                | Self::ResourceNotFound
                | Self::DuplicateResource
                | Self::InvalidState
                | Self::OperationFailed
                | Self::PermissionDenied
        )
    }
}

/// Tool group for isolation of tool suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGroup {
    Browser,
    Planning,
    Scratchpad,
    Workspace,
    Agent, // Unified Agent Domain (Assistant/Swarm)
    Attachments,
    Knowledge,
    Playbook,
    UI,
    ScheduledTask,
    Tool, // Unified Tool Domain (MCP Manager)
    Bootstrap,
}
