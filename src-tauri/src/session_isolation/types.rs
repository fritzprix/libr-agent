use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Shell type enumeration for cross-platform shell support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Used by tool handlers and future shell selection logic
pub enum ShellType {
    Bash,
    PowerShell,
    Cmd,
}

#[derive(Debug, Clone)]
pub struct IsolationConfig {
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    #[allow(dead_code)] // Planned for future use
    pub max_memory_mb: Option<u64>,
    #[allow(dead_code)] // Planned for future use
    pub max_execution_time_secs: Option<u64>,
    #[allow(dead_code)] // Planned for future use
    pub max_open_files: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct IsolatedProcessConfig {
    pub session_id: String,
    pub workspace_path: PathBuf,
    pub command: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub isolation_level: IsolationLevel,
    pub shell_type: Option<ShellType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IsolationLevel {
    /// Basic process isolation (environment variables only)
    #[allow(dead_code)] // Reserved for future use
    Basic,
    /// Medium isolation (process groups + limited resources)
    Medium,
    /// High isolation (platform-specific sandboxing)
    #[allow(dead_code)] // Reserved for future use
    High,
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            resource_limits: ResourceLimits {
                max_memory_mb: Some(512),
                max_execution_time_secs: Some(300),
                max_open_files: Some(1024),
            },
        }
    }
}
