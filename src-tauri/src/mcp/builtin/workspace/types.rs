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
