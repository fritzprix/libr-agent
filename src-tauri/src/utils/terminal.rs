use std::path::Path;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::process::Command;

/// Opens the specified path in a system terminal.
///
/// This function handles the OS-specific commands to open a terminal at the given path.
///
/// # Arguments
/// * `path` - The path to open in the terminal.
///
/// # Returns
/// * `Result<(), String>` - Ok if the terminal was spawned successfully, Err otherwise.
pub fn open_in_terminal(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args([
                "/c",
                "start",
                "Agent Workspace",
                "/D",
                &path.to_string_lossy(),
                "cmd",
                "/k",
            ])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let path_str = path.to_string_lossy();
        // Quote for shell
        let shell_quoted = format!("'{}'", path_str.replace("'", "'\\''"));
        // Escape for AppleScript string
        let script_cmd = format!("cd {}", shell_quoted);
        let applescript_escaped = script_cmd.replace("\\", "\\\\").replace("\"", "\\\"");

        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            applescript_escaped
        );

        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to open Terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        return Err(format!(
            "Terminal launch not supported on Linux. No standard command available. \
             Open a terminal manually and navigate to: {}",
            path.display()
        ));
    }

    // For Windows and macOS, the terminal was already launched above
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        Ok(())
    }

    // Fallback for unsupported platforms (unlikely, but comprehensive)
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Terminal launch not supported on this platform".to_string())
    }
}
