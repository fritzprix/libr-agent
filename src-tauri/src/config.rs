/// Centralized configuration management for LibrAgent
///
/// This module provides environment-driven configuration with fallback defaults.
/// All configuration values can be overridden via environment variables.
///
/// # Development Mode
/// In debug builds, environment variables are loaded from a `.env` file in the `src-tauri/` directory
/// (if it exists). Create a `.env` file in `src-tauri/` for local development configuration.
///
/// # Production Mode
/// In release builds, only system environment variables are used. Configure your
/// deployment environment accordingly.
///
/// # Available Environment Variables
/// - `LIBRAGENT_MAX_FILE_SIZE`: Maximum file size in bytes (default: 104857600 = 100MB)
/// - `LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT`: Default command timeout in seconds (default: 30)
/// - `LIBRAGENT_MAX_EXECUTION_TIMEOUT`: Maximum command timeout in seconds (default: 300)
/// - `LIBRAGENT_MAX_OUTPUT_SIZE`: Maximum process output size in bytes (default: 104857600 = 100MB)
/// - `LIBRAGENT_GRACEFUL_SHUTDOWN_TIMEOUT`: Graceful shutdown timeout in seconds (default: 3)
/// - `LIBRAGENT_POLL_THRESHOLD`: Excessive polling detection threshold (default: 5 consecutive polls)
/// - `MESSAGE_INDEX_SNIPPET_LENGTH`: Message snippet length for search index (default: 200)
/// - `LIBRAGENT_DB_PATH`: SQLite database file path (default: user data directory)
/// - `LIBRAGENT_MCP_IDLE_TIMEOUT_MINUTES`: MCP server idle timeout in minutes (default: 5)
/// - `LIBRAGENT_MCP_CLEANUP_INTERVAL_MINUTES`: MCP cleanup interval in minutes (default: 5)
/// - `LIBRAGENT_MCP_STARTUP_TIMEOUT_SECONDS`: MCP server startup timeout in seconds (default: 10)
use std::env;

/// Default maximum file size (100 MB)
const DEFAULT_MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Default execution timeout (30 seconds)
const DEFAULT_EXECUTION_TIMEOUT: u64 = 30;

/// Default maximum execution timeout (5 minutes)
const DEFAULT_MAX_EXECUTION_TIMEOUT: u64 = 300;

/// Default snippet length for message index (200 characters)
const DEFAULT_SNIPPET_LENGTH: usize = 200;

/// Default maximum captured output size for spawned processes (100 MB)
const DEFAULT_MAX_OUTPUT_SIZE: u64 = 100 * 1024 * 1024;

/// Default polling threshold for excessive polling detection (5 consecutive polls)
const DEFAULT_POLL_THRESHOLD: u32 = 5;

/// Get maximum output size for process stdout/stderr capture from environment or use default
///
/// Environment variable: LIBRAGENT_MAX_OUTPUT_SIZE
/// Default: 104857600 (100 MB)
pub fn max_output_size() -> u64 {
    env::var("LIBRAGENT_MAX_OUTPUT_SIZE")
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
/// Environment variable: LIBRAGENT_GRACEFUL_SHUTDOWN_TIMEOUT
pub fn graceful_shutdown_timeout() -> u64 {
    env::var("LIBRAGENT_GRACEFUL_SHUTDOWN_TIMEOUT")
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
/// Environment variable: LIBRAGENT_MAX_FILE_SIZE
/// Default: 104857600 (100 MB)
pub fn max_file_size() -> usize {
    env::var("LIBRAGENT_MAX_FILE_SIZE")
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
/// Environment variable: LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT
/// Default: 30 seconds
pub fn default_execution_timeout() -> u64 {
    env::var("LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT")
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
/// Environment variable: LIBRAGENT_MAX_EXECUTION_TIMEOUT
/// Default: 300 seconds (5 minutes)
pub fn max_execution_timeout() -> u64 {
    let max_timeout = env::var("LIBRAGENT_MAX_EXECUTION_TIMEOUT")
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

/// Get polling threshold for excessive polling detection from environment or use default
///
/// Environment variable: LIBRAGENT_POLL_THRESHOLD
/// Default: 5 consecutive polls while process is running
pub fn poll_threshold() -> u32 {
    env::var("LIBRAGENT_POLL_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default poll threshold: {} consecutive polls",
                DEFAULT_POLL_THRESHOLD
            );
            DEFAULT_POLL_THRESHOLD
        })
}

/// Default MCP server idle timeout (5 minutes)
const DEFAULT_MCP_IDLE_TIMEOUT_MINUTES: u64 = 5;

/// Get MCP server idle timeout in minutes from environment or use default
///
/// Environment variable: LIBRAGENT_MCP_IDLE_TIMEOUT_MINUTES
/// Default: 5 minutes
pub fn mcp_idle_timeout_minutes() -> u64 {
    env::var("LIBRAGENT_MCP_IDLE_TIMEOUT_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default MCP idle timeout: {} minutes",
                DEFAULT_MCP_IDLE_TIMEOUT_MINUTES
            );
            DEFAULT_MCP_IDLE_TIMEOUT_MINUTES
        })
}

/// Default MCP cleanup interval (5 minutes)
const DEFAULT_MCP_CLEANUP_INTERVAL_MINUTES: u64 = 5;

/// Get MCP cleanup interval in minutes from environment or use default
///
/// Environment variable: LIBRAGENT_MCP_CLEANUP_INTERVAL_MINUTES
/// Default: 5 minutes
pub fn mcp_cleanup_interval_minutes() -> u64 {
    env::var("LIBRAGENT_MCP_CLEANUP_INTERVAL_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default MCP cleanup interval: {} minutes",
                DEFAULT_MCP_CLEANUP_INTERVAL_MINUTES
            );
            DEFAULT_MCP_CLEANUP_INTERVAL_MINUTES
        })
}

/// Default MCP server startup timeout (10 seconds)
const DEFAULT_MCP_STARTUP_TIMEOUT_SECONDS: u64 = 10;

/// Get MCP server startup timeout in seconds from environment or use default
///
/// Environment variable: LIBRAGENT_MCP_STARTUP_TIMEOUT_SECONDS
/// Default: 10 seconds
pub fn mcp_startup_timeout_seconds() -> u64 {
    env::var("LIBRAGENT_MCP_STARTUP_TIMEOUT_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            tracing::debug!(
                "Using default MCP startup timeout: {} seconds",
                DEFAULT_MCP_STARTUP_TIMEOUT_SECONDS
            );
            DEFAULT_MCP_STARTUP_TIMEOUT_SECONDS
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
