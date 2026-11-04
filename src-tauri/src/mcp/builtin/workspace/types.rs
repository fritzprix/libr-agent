use serde::{Deserialize, Serialize};

/// Platform-specific persistent shell tool name
#[cfg(unix)]
pub const PERSISTENT_SHELL_TOOL: &str = "runInPersistentShell";
#[cfg(windows)]
pub const PERSISTENT_SHELL_TOOL: &str = "runInPersistentPowerShell";

/// Platform-specific one-shot shell tool name
#[cfg(unix)]
pub const RUN_SHELL_TOOL: &str = "runShell";
#[cfg(windows)]
pub const RUN_SHELL_TOOL: &str = "runPowerShell";

pub(crate) const INTERACTIVE_SHELL_INPUT_MAX_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InteractiveShellInputType {
    Text,
    Password,
}

impl InteractiveShellInputType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Password => "password",
        }
    }
}

impl std::fmt::Display for InteractiveShellInputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How interactive shell input is delivered to the executed command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StdinDelivery {
    /// Feed input as the next line on the persistent shell stdin (Read-Host, sudo).
    Host,
    /// Pipe input into the command so child processes can read sys.stdin (python --code-stdin).
    Child,
}

/// A shell command that waits for one line of user input before it runs.
#[derive(Debug, Clone)]
pub struct InteractiveShellRequest {
    pub session_id: String,
    pub executable_command: String, // Command to execute (may include -S flag)
    pub display_command: String,    // Sanitized version for logs/UI
    pub run_mode: String,           // "sync" or "async" from 1st call
    pub timeout: u64,               // Command execution timeout in seconds
    pub input_type: InteractiveShellInputType,
    pub stdin_delivery: StdinDelivery,
}
