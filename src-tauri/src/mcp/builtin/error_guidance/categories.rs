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
    /// Returns true when the failure is attributable to tool misuse or invalid tool input.
    pub fn uses_error_semantics(self) -> bool {
        matches!(
            self,
            Self::MissingRequiredParam | Self::InvalidInput | Self::InvalidFormat
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
    Tool, // Unified Tool Domain (MCP Manager)
    Bootstrap,
}
