use crate::session_isolation::types::ShellType;
use std::path::PathBuf;
use tokio::process::Command as AsyncCommand;
use tracing::warn;

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

/// Get restricted PATH for security
#[allow(dead_code)] // Used only on Unix platforms
pub fn get_restricted_path() -> String {
    if cfg!(target_os = "windows") {
        // Windows PATH must include:
        // - System32: Core Windows commands (cmd, findstr, etc.)
        // - Windows: Additional system utilities
        // - System32\WindowsPowerShell\v1.0: PowerShell (if available)
        // Note: We intentionally restrict access to user-installed software
        "C:\\Windows\\System32;C:\\Windows;C:\\Windows\\System32\\WindowsPowerShell\\v1.0"
            .to_string()
    } else {
        // Unix: Include common user installation paths
        // - /bin, /usr/bin: System binaries
        // - /usr/local/bin: User-installed software (brew, etc.)
        // - ~/.local/bin: Python pip, pipx, uv, etc.
        // - ~/.cargo/bin: Rust cargo-installed tools
        let mut paths = vec![
            "/bin".to_string(),
            "/usr/bin".to_string(),
            "/usr/local/bin".to_string(),
        ];

        // Add user-specific paths if HOME is available
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            paths.push(
                home_path
                    .join(".local")
                    .join("bin")
                    .to_string_lossy()
                    .to_string(),
            );
            paths.push(
                home_path
                    .join(".cargo")
                    .join("bin")
                    .to_string_lossy()
                    .to_string(),
            );
        }

        paths.join(":")
    }
}
