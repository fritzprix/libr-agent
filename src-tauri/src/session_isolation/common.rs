use tokio::process::Command as AsyncCommand;
use tracing::warn;
use crate::session_isolation::types::ShellType;

/// Check if a command is available on the system
#[allow(dead_code)] // Used by platform-specific high isolation
pub async fn is_command_available(command: &str) -> bool {
    // Use the async Tokio Command to avoid blocking the async runtime
    let mut cmd = if cfg!(target_os = "windows") {
        AsyncCommand::new("where")
    } else {
        AsyncCommand::new("which")
    };

    cmd.arg(command);

    match cmd.output().await {
        Ok(output) => output.status.success(),
        Err(err) => {
            warn!("Failed to check command availability: {err}");
            false
        }
    }
}

/// Get the appropriate shell command for the platform and shell type
pub fn get_shell_command(shell_type: Option<ShellType>) -> &'static str {
    if cfg!(target_os = "windows") {
        match shell_type {
            Some(ShellType::Cmd) => "cmd",
            Some(ShellType::PowerShell) | Some(ShellType::Bash) | None => "powershell",
        }
    } else {
        "bash"
    }
}
