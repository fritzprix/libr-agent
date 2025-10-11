/// Centralized configuration management for SynapticFlow
///
/// This module provides environment-driven configuration with fallback defaults.
/// All configuration values can be overridden via environment variables.
///
/// # Development Mode
/// In debug builds, environment variables are loaded from a `.env` file in the project root
/// (if it exists). See `.env.example` for available configuration options.
///
/// # Production Mode
/// In release builds, only system environment variables are used. Configure your
/// deployment environment accordingly.
///
/// # Available Environment Variables
/// - `SYNAPTICFLOW_MAX_FILE_SIZE`: Maximum file size in bytes (default: 10485760 = 10MB)
/// - `SYNAPTICFLOW_DEFAULT_EXECUTION_TIMEOUT`: Default command timeout in seconds (default: 30)
/// - `SYNAPTICFLOW_MAX_EXECUTION_TIMEOUT`: Maximum command timeout in seconds (default: 300)
/// - `MESSAGE_INDEX_SNIPPET_LENGTH`: Message snippet length for search index (default: 200)
/// - `SYNAPTICFLOW_DB_PATH`: SQLite database file path (default: user data directory)
use std::env;

/// Default maximum file size (10 MB)
const DEFAULT_MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Default execution timeout (30 seconds)
const DEFAULT_EXECUTION_TIMEOUT: u64 = 30;

/// Default maximum execution timeout (5 minutes)
const DEFAULT_MAX_EXECUTION_TIMEOUT: u64 = 300;

/// Default snippet length for message index (200 characters)
const DEFAULT_SNIPPET_LENGTH: usize = 200;

/// Default maximum captured output size for spawned processes (100 MB)
const DEFAULT_MAX_OUTPUT_SIZE: u64 = 100 * 1024 * 1024;

/// Get maximum output size for process stdout/stderr capture from environment or use default
///
/// Environment variable: SYNAPTICFLOW_MAX_OUTPUT_SIZE
/// Default: 104857600 (100 MB)
pub fn max_output_size() -> u64 {
    env::var("SYNAPTICFLOW_MAX_OUTPUT_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default max output size: {} bytes",
                DEFAULT_MAX_OUTPUT_SIZE
            );
            DEFAULT_MAX_OUTPUT_SIZE
        })
}

/// Default graceful shutdown timeout in seconds
const DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT: u64 = 3;

/// Get graceful shutdown timeout (seconds) from environment or default
///
/// Environment variable: SYNAPTICFLOW_GRACEFUL_SHUTDOWN_TIMEOUT
pub fn graceful_shutdown_timeout() -> u64 {
    env::var("SYNAPTICFLOW_GRACEFUL_SHUTDOWN_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default graceful shutdown timeout: {} seconds",
                DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT
            );
            DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT
        })
}

/// Get maximum file size from environment or use default
///
/// Environment variable: SYNAPTICFLOW_MAX_FILE_SIZE
/// Default: 10485760 (10 MB)
pub fn max_file_size() -> usize {
    env::var("SYNAPTICFLOW_MAX_FILE_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default max file size: {} bytes",
                DEFAULT_MAX_FILE_SIZE
            );
            DEFAULT_MAX_FILE_SIZE
        })
}

/// Get default execution timeout from environment or use default
///
/// Environment variable: SYNAPTICFLOW_DEFAULT_EXECUTION_TIMEOUT
/// Default: 30 seconds
pub fn default_execution_timeout() -> u64 {
    env::var("SYNAPTICFLOW_DEFAULT_EXECUTION_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default execution timeout: {} seconds",
                DEFAULT_EXECUTION_TIMEOUT
            );
            DEFAULT_EXECUTION_TIMEOUT
        })
}

/// Get maximum execution timeout from environment or use default
///
/// Environment variable: SYNAPTICFLOW_MAX_EXECUTION_TIMEOUT
/// Default: 300 seconds (5 minutes)
pub fn max_execution_timeout() -> u64 {
    let max_timeout = env::var("SYNAPTICFLOW_MAX_EXECUTION_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default max execution timeout: {} seconds",
                DEFAULT_MAX_EXECUTION_TIMEOUT
            );
            DEFAULT_MAX_EXECUTION_TIMEOUT
        });

    // Ensure max timeout is at least as large as default timeout
    let default_timeout = default_execution_timeout();
    if max_timeout < default_timeout {
        tracing::warn!(
            "Max execution timeout ({}) is less than default timeout ({}). Using default as max.",
            max_timeout,
            default_timeout
        );
        default_timeout
    } else {
        max_timeout
    }
}

/// Get message index snippet length from environment or use default
///
/// Environment variable: MESSAGE_INDEX_SNIPPET_LENGTH
/// Default: 200 characters
pub fn message_index_snippet_length() -> usize {
    env::var("MESSAGE_INDEX_SNIPPET_LENGTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default snippet length: {} characters",
                DEFAULT_SNIPPET_LENGTH
            );
            DEFAULT_SNIPPET_LENGTH
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_when_no_env() {
        // These tests assume no environment variables are set
        // In a real test environment, you might want to use a library like `temp-env`
        assert_eq!(max_file_size(), DEFAULT_MAX_FILE_SIZE);
        assert_eq!(default_execution_timeout(), DEFAULT_EXECUTION_TIMEOUT);
        assert_eq!(max_execution_timeout(), DEFAULT_MAX_EXECUTION_TIMEOUT);
        assert_eq!(message_index_snippet_length(), DEFAULT_SNIPPET_LENGTH);
    }

    #[test]
    fn test_max_timeout_validation() {
        // max_execution_timeout should be at least as large as default_execution_timeout
        let max = max_execution_timeout();
        let default = default_execution_timeout();
        assert!(max >= default);
    }
}
