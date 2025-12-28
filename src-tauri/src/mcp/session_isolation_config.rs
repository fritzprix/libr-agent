use serde::{Deserialize, Serialize};

/// Configuration for session isolation features in MCP server management.
///
/// This struct holds settings that control how external MCP servers are managed
/// on a per-session basis, including process lifecycle, timeouts, and resource cleanup.
///
/// ## Environment Variables
///
/// All settings can be configured via environment variables:
/// - `LIBRAGENT_MCP_IDLE_TIMEOUT_MINUTES` - Idle timeout (default: 5 minutes)
/// - `LIBRAGENT_MCP_CLEANUP_INTERVAL_MINUTES` - Cleanup interval (default: 5 minutes)
/// - `LIBRAGENT_MCP_STARTUP_TIMEOUT_SECONDS` - Startup timeout (default: 10 seconds)
///
/// ## Example
///
/// ```bash
/// # Set 10 minute idle timeout
/// export LIBRAGENT_MCP_IDLE_TIMEOUT_MINUTES=10
///
/// # Set 2 minute cleanup interval
/// export LIBRAGENT_MCP_CLEANUP_INTERVAL_MINUTES=2
///
/// # Set 30 second startup timeout
/// export LIBRAGENT_MCP_STARTUP_TIMEOUT_SECONDS=30
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIsolationConfig {
    /// Idle timeout in minutes before an MCP server process is terminated.
    /// Default: 5 minutes
    /// Environment variable: `LIBRAGENT_MCP_IDLE_TIMEOUT_MINUTES`
    pub idle_timeout_minutes: u64,

    /// Interval in minutes between cleanup task runs.
    /// Default: 5 minutes
    /// Environment variable: `LIBRAGENT_MCP_CLEANUP_INTERVAL_MINUTES`
    pub cleanup_interval_minutes: u64,

    /// Timeout in seconds for MCP server process startup/initialization.
    /// Default: 10 seconds
    /// Environment variable: `LIBRAGENT_MCP_STARTUP_TIMEOUT_SECONDS`
    pub process_startup_timeout_seconds: u64,

    /// Maximum number of restart attempts for crashed processes.
    /// Default: 0 (no automatic restart)
    pub max_restart_attempts: u32,

    /// Size of HTTP connection pool per server.
    /// Default: 10 connections
    pub http_connection_pool_size: usize,
}

impl Default for SessionIsolationConfig {
    /// Creates a configuration with values from environment variables, falling back to defaults.
    fn default() -> Self {
        Self {
            idle_timeout_minutes: crate::config::mcp_idle_timeout_minutes(),
            cleanup_interval_minutes: crate::config::mcp_cleanup_interval_minutes(),
            process_startup_timeout_seconds: crate::config::mcp_startup_timeout_seconds(),
            max_restart_attempts: 0, // No auto-restart by default
            http_connection_pool_size: 10,
        }
    }
}

impl SessionIsolationConfig {
    /// Creates a new configuration with custom idle timeout.
    pub fn with_idle_timeout(mut self, minutes: u64) -> Self {
        self.idle_timeout_minutes = minutes;
        self
    }

    /// Creates a new configuration with custom cleanup interval.
    pub fn with_cleanup_interval(mut self, minutes: u64) -> Self {
        self.cleanup_interval_minutes = minutes;
        self
    }

    /// Creates a new configuration with custom startup timeout.
    pub fn with_startup_timeout(mut self, seconds: u64) -> Self {
        self.process_startup_timeout_seconds = seconds;
        self
    }
}
