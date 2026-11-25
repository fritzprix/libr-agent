//! Errors of an LLM call and whether a retry can help.

use std::fmt;

/// What went wrong, as far as the caller needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The provider could not be reached, or the connection broke.
    Network,
    /// The provider rejected the credentials (HTTP 401 or 403).
    Authentication,
    /// The provider limited the request rate (HTTP 429).
    RateLimit,
    /// The provider answered with an error, or a stream that could not be read.
    Service,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmError {
    pub kind: ErrorKind,
    /// Stable code for logs and the UI.
    pub code: &'static str,
    pub message: String,
    /// HTTP status, when the provider sent one.
    pub status: Option<u16>,
}

/// How much of an error body is kept for the user-facing message.
const ERROR_BODY_CHARS: usize = 600;

impl LlmError {
    pub fn new(kind: ErrorKind, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
            status: None,
        }
    }

    /// The error for an HTTP response that was not a success.
    pub fn from_http_status(status: u16, body: &str) -> Self {
        let snippet: String = body.chars().take(ERROR_BODY_CHARS).collect();
        let message = format!("LLM provider returned HTTP {status}: {snippet}");
        let (kind, code) = match status {
            401 | 403 => (ErrorKind::Authentication, "RUST_EXECUTOR_AUTH"),
            429 => (ErrorKind::RateLimit, "RUST_EXECUTOR_RATE_LIMIT"),
            _ => (ErrorKind::Service, "RUST_EXECUTOR_HTTP"),
        };
        Self {
            status: Some(status),
            ..Self::new(kind, code, message)
        }
    }

    /// True when the same request may succeed later: network failures, server
    /// errors, and rate limits that are not a spending cap. Same rule as the
    /// GUI's retry policy.
    pub fn is_retryable(&self) -> bool {
        match self.kind {
            ErrorKind::Network => true,
            ErrorKind::RateLimit => !self.is_spending_cap(),
            ErrorKind::Service => self.status.is_some_and(|status| status >= 500),
            ErrorKind::Authentication => false,
        }
    }

    fn is_spending_cap(&self) -> bool {
        let lower = self.message.to_lowercase();
        lower.contains("spending cap")
            || (self.message.contains("RESOURCE_EXHAUSTED") && lower.contains("spending"))
    }
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LlmError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_statuses_are_classified() {
        let auth = LlmError::from_http_status(401, "bad key");
        assert_eq!(auth.kind, ErrorKind::Authentication);
        assert!(auth.message.contains("HTTP 401: bad key"));
        assert_eq!(
            LlmError::from_http_status(429, "").kind,
            ErrorKind::RateLimit
        );
        assert_eq!(LlmError::from_http_status(502, "").kind, ErrorKind::Service);
    }

    #[test]
    fn only_transient_failures_are_retried() {
        assert!(LlmError::new(ErrorKind::Network, "X", "down").is_retryable());
        assert!(LlmError::from_http_status(503, "").is_retryable());
        assert!(LlmError::from_http_status(429, "slow down").is_retryable());
        assert!(!LlmError::from_http_status(429, "You exceeded your spending cap").is_retryable());
        assert!(!LlmError::from_http_status(400, "bad request").is_retryable());
        assert!(!LlmError::from_http_status(401, "").is_retryable());
        // A stream that broke after it started is not retried.
        assert!(!LlmError::new(ErrorKind::Service, "X", "malformed").is_retryable());
    }

    #[test]
    fn a_long_error_body_is_cut() {
        let error = LlmError::from_http_status(500, &"x".repeat(5_000));
        assert!(error.message.len() < 700);
    }
}
