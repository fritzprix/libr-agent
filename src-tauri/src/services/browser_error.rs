use serde::{Deserialize, Serialize};
use std::fmt;

/// Structured error types for browser operations
/// This ensures type-safe error handling between Rust and TypeScript
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "code", content = "context")]
pub enum BrowserError {
    /// Session with the given ID was not found
    #[serde(rename = "SESSION_NOT_FOUND")]
    SessionNotFound { session_id: String },

    /// Session was already closed
    #[serde(rename = "SESSION_CLOSED")]
    SessionClosed { session_id: String },

    /// Browser window was not found
    #[serde(rename = "WINDOW_NOT_FOUND")]
    WindowNotFound { session_id: String },

    /// Element could not be found with the given selector
    #[serde(rename = "ELEMENT_NOT_FOUND")]
    ElementNotFound {
        selector: String,
        session_id: String,
    },

    /// Element exists but is not interactable
    #[serde(rename = "ELEMENT_NOT_INTERACTABLE")]
    ElementNotInteractable {
        selector: String,
        reason: String,
        session_id: String,
    },

    /// Navigation failed
    #[serde(rename = "NAVIGATION_FAILED")]
    NavigationFailed {
        url: String,
        reason: String,
        session_id: String,
    },

    /// Script execution failed
    #[serde(rename = "SCRIPT_EXECUTION_FAILED")]
    ScriptExecutionFailed { reason: String, session_id: String },

    /// Operation timed out
    #[serde(rename = "TIMEOUT")]
    Timeout {
        operation: String,
        duration_ms: u64,
        session_id: String,
    },

    /// Lock acquisition failed
    #[serde(rename = "LOCK_FAILED")]
    LockFailed { reason: String },

    /// Invalid parameter
    #[serde(rename = "INVALID_PARAMETER")]
    InvalidParameter { parameter: String, reason: String },

    /// Generic error
    #[serde(rename = "UNKNOWN")]
    Unknown { message: String },
}

impl BrowserError {
    /// Convert error to user-friendly message
    pub fn to_message(&self) -> String {
        match self {
            BrowserError::SessionNotFound { session_id } => {
                format!("Session '{session_id}' not found")
            }
            BrowserError::SessionClosed { session_id } => {
                format!("Session '{session_id}' was already closed")
            }
            BrowserError::WindowNotFound { session_id } => {
                format!("Browser window for session '{session_id}' not found")
            }
            BrowserError::ElementNotFound {
                selector,
                session_id,
            } => {
                format!("Element with selector '{selector}' not found in session '{session_id}'")
            }
            BrowserError::ElementNotInteractable {
                selector,
                reason,
                session_id,
            } => {
                format!(
                    "Element '{selector}' in session '{session_id}' is not interactable: {reason}"
                )
            }
            BrowserError::NavigationFailed {
                url,
                reason,
                session_id,
            } => {
                format!("Navigation to '{url}' failed in session '{session_id}': {reason}")
            }
            BrowserError::ScriptExecutionFailed { reason, session_id } => {
                format!("Script execution failed in session '{session_id}': {reason}")
            }
            BrowserError::Timeout {
                operation,
                duration_ms,
                session_id,
            } => {
                format!(
                    "Operation '{operation}' timed out after {duration_ms}ms in session '{session_id}'"
                )
            }
            BrowserError::LockFailed { reason } => {
                format!("Failed to acquire lock: {reason}")
            }
            BrowserError::InvalidParameter { parameter, reason } => {
                format!("Invalid parameter '{parameter}': {reason}")
            }
            BrowserError::Unknown { message } => message.clone(),
        }
    }

    /// Get error code as string
    pub fn code(&self) -> &'static str {
        match self {
            BrowserError::SessionNotFound { .. } => "SESSION_NOT_FOUND",
            BrowserError::SessionClosed { .. } => "SESSION_CLOSED",
            BrowserError::WindowNotFound { .. } => "WINDOW_NOT_FOUND",
            BrowserError::ElementNotFound { .. } => "ELEMENT_NOT_FOUND",
            BrowserError::ElementNotInteractable { .. } => "ELEMENT_NOT_INTERACTABLE",
            BrowserError::NavigationFailed { .. } => "NAVIGATION_FAILED",
            BrowserError::ScriptExecutionFailed { .. } => "SCRIPT_EXECUTION_FAILED",
            BrowserError::Timeout { .. } => "TIMEOUT",
            BrowserError::LockFailed { .. } => "LOCK_FAILED",
            BrowserError::InvalidParameter { .. } => "INVALID_PARAMETER",
            BrowserError::Unknown { .. } => "UNKNOWN",
        }
    }
}

impl fmt::Display for BrowserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_message())
    }
}

impl From<BrowserError> for String {
    fn from(error: BrowserError) -> Self {
        // Serialize to JSON for structured error passing
        serde_json::to_string(&error).unwrap_or_else(|_| error.to_message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_serialization() {
        let error = BrowserError::SessionNotFound {
            session_id: "test-123".to_string(),
        };

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("SESSION_NOT_FOUND"));
        assert!(json.contains("test-123"));

        let deserialized: BrowserError = serde_json::from_str(&json).unwrap();
        assert_eq!(error.code(), deserialized.code());
    }

    #[test]
    fn test_error_messages() {
        let error = BrowserError::ElementNotFound {
            selector: ".button".to_string(),
            session_id: "sess-1".to_string(),
        };

        assert!(error.to_message().contains(".button"));
        assert!(error.to_message().contains("sess-1"));
    }
}
