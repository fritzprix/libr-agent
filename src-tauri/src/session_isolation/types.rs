use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Shell type enumeration for cross-platform shell support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Sh,
    PowerShell,
}

impl ShellType {
    /// Get shell command for spawning
    pub fn command(&self) -> &str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Sh => "sh",
            ShellType::PowerShell => "powershell.exe",
        }
    }

    /// Check if this is a Windows shell
    pub fn is_windows(&self) -> bool {
        matches!(self, ShellType::PowerShell)
    }
}

#[derive(Debug, Clone)]
pub struct IsolatedProcessConfig {
    pub session_id: String,
    pub workspace_path: PathBuf,
    /// Optional process working directory. Defaults to `workspace_path` when `None`.
    /// Must stay under the session workspace for host isolation.
    pub working_directory: Option<PathBuf>,
    pub command: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub isolation_level: IsolationLevel,
    pub shell_type: Option<ShellType>,
}

impl IsolatedProcessConfig {
    /// Directory used for `Command::current_dir`.
    pub fn effective_working_directory(&self) -> &PathBuf {
        self.working_directory
            .as_ref()
            .unwrap_or(&self.workspace_path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IsolationLevel {
    /// Basic process isolation (environment variables only)
    Basic,
    /// Medium isolation (process groups + limited resources)
    Medium,
    /// High isolation (platform-specific sandboxing)
    High,
}
