use thiserror::Error;

/// Errors that can occur during session-specific MCP server management.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum SessionMCPError {
    /// The requested MCP server was not found in the session's configuration.
    #[error("Server not found: {0}")]
    ServerNotFound(String),

    /// The server's transport type is invalid for the requested operation.
    #[error("Invalid transport type: {0}")]
    InvalidTransport(String),

    /// Failed to spawn the MCP server process.
    #[error("Process spawn failed: {0}")]
    SpawnFailed(String),

    /// The MCP server process crashed during operation.
    #[error("Process crashed: {server} - {error}")]
    ProcessCrashed { server: String, error: String },

    /// Process initialization timed out.
    #[error("Process initialization timeout: {0}")]
    InitTimeout(String),

    /// Process initialization failed.
    #[error("Process initialization failed: {0}")]
    InitFailed(String),

    /// Tool call was cancelled.
    #[error("Tool call cancelled")]
    CallCancelled,

    /// Failed to serialize or deserialize data.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// The session has been closed.
    #[error("Session closed")]
    SessionClosed,

    /// Tool call failed.
    #[error("Tool call failed: {0}")]
    ToolCallFailed(String),

    /// Execution error during tool call
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),
}
