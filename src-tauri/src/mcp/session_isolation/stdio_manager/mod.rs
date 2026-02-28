use super::process::MCPProcess;
use crate::mcp::session_isolation_config::SessionIsolationConfig;
use crate::mcp::types::MCPServerConfig;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

mod cleanup;
mod execution;
mod lifecycle;
#[cfg(test)]
mod tests;

/// Manages session-specific MCP server processes with lazy spawning and idle cleanup.
#[derive(Debug, Clone)]
pub struct SessionMCPManager {
    /// Unique session identifier.
    pub(crate) session_id: String,

    /// Map of server names to running processes.
    pub(crate) active_processes: Arc<RwLock<HashMap<String, MCPProcess>>>,

    /// Map of server names to last activity timestamps.
    pub(crate) last_activity: Arc<RwLock<HashMap<String, Instant>>>,

    /// Per-server spawn locks to prevent race conditions.
    pub(crate) spawn_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,

    /// Idle timeout duration.
    pub(crate) idle_timeout: Duration,

    /// Server configurations for this session.
    pub(crate) server_configs: HashMap<String, MCPServerConfig>,

    /// Session isolation configuration.
    pub(crate) config: SessionIsolationConfig,

    /// Cancellation tokens for active calls.
    pub(crate) active_call_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,

    /// Workspace directory for the session (CWD for child processes)
    pub(crate) workspace_dir: std::path::PathBuf,
}

impl SessionMCPManager {
    /// Checks if a server is managed by this session manager
    pub fn has_server(&self, server_name: &str) -> bool {
        self.server_configs.contains_key(server_name)
    }
}
