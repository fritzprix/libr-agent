use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
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

#[derive(Clone)]
pub struct IsolatedProcessConfig {
    pub session_id: String,
    pub workspace_path: PathBuf,
    /// Optional process working directory. Defaults to `workspace_path` when `None`.
    /// Must stay under the session workspace for host isolation.
    pub working_directory: Option<PathBuf>,
    pub command: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    /// Session `storeAs` values. Applied before every other variable, so they never
    /// override one the app or the call sets; Docker gets them as `-e KEY` only.
    pub secrets: BTreeMap<String, String>,
    pub isolation_level: IsolationLevel,
    pub shell_type: Option<ShellType>,
    /// Run as uid 0. Only a LibrAgent-managed Docker session container allows it.
    pub run_as_root: bool,
}

impl std::fmt::Debug for IsolatedProcessConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IsolatedProcessConfig")
            .field("session_id", &self.session_id)
            .field("workspace_path", &self.workspace_path)
            .field("working_directory", &self.working_directory)
            .field("command", &self.command)
            .field("args", &self.args)
            .field("env_vars", &self.env_vars)
            .field("secret_names", &self.secrets.keys().collect::<Vec<_>>())
            .field("isolation_level", &self.isolation_level)
            .field("shell_type", &self.shell_type)
            .field("run_as_root", &self.run_as_root)
            .finish()
    }
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
