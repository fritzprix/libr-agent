/// Utility functions for cross-platform command execution
///
/// Windows requires special handling for .cmd, .bat, and .ps1 files,
/// which need to be executed through cmd.exe or powershell.exe
use std::path::Path;

/// Determines if a command needs shell wrapping on Windows
///
/// On Windows, script files (.cmd, .bat, .ps1) cannot be executed directly
/// and must be invoked through cmd.exe or powershell.exe
fn needs_shell_wrapper(command: &str) -> bool {
    #[cfg(windows)]
    {
        // If already an .exe, no wrapper needed
        if command.ends_with(".exe") {
            return false;
        }

        // Check if it's a known script extension
        if command.ends_with(".cmd") || command.ends_with(".bat") || command.ends_with(".ps1") {
            return true;
        }

        // For commands without extension, check if it's a common Node.js/Python tool
        // These are typically .cmd files on Windows
        let basename = Path::new(command)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(command);

        matches!(
            basename,
            "npx" | "npm" | "node" | "pnpm" | "yarn" | "bun" | // Node.js ecosystem
            "uvx" | "uv" | "pip" | "pipx" | // Python ecosystem
            "python" | "python3" // Python interpreters
        )
    }

    #[cfg(not(windows))]
    {
        let _ = command;
        false
    }
}

/// Wraps a command with appropriate shell on Windows if needed
///
/// Returns (final_command, final_args) tuple:
/// - On Windows with .cmd/.bat files: ("cmd.exe", ["/C", original_command, ...args])
/// - On Windows with .ps1 files: ("powershell.exe", ["-File", original_command, ...args])
/// - Otherwise: (original_command, args)
///
/// # Arguments
/// * `command` - The command to execute
/// * `args` - Command arguments
///
/// # Returns
/// Tuple of (final_command, final_args) ready for Command::new()
pub fn prepare_command(command: &str, args: &[String]) -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        if needs_shell_wrapper(command) {
            // PowerShell scripts need powershell.exe
            if command.ends_with(".ps1") {
                let mut new_args = vec![
                    "-ExecutionPolicy".to_string(),
                    "Bypass".to_string(),
                    "-File".to_string(),
                    command.to_string(),
                ];
                new_args.extend(args.iter().cloned());
                return ("powershell.exe".to_string(), new_args);
            }

            // Everything else (including .cmd, .bat, and Node.js tools) uses cmd.exe
            // Use /C instead of /c (though both work, /C is more idiomatic)
            let mut new_args = vec!["/C".to_string(), command.to_string()];
            new_args.extend(args.iter().cloned());
            return ("cmd.exe".to_string(), new_args);
        }
    }

    // No wrapping needed (Unix/Linux or Windows .exe)
    (command.to_string(), args.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_needs_shell_wrapper_windows() {
        // .exe files don't need wrapper
        assert!(!needs_shell_wrapper("program.exe"));

        // Explicit script extensions need wrapper
        assert!(needs_shell_wrapper("script.cmd"));
        assert!(needs_shell_wrapper("script.bat"));
        assert!(needs_shell_wrapper("script.ps1"));

        // Known Node.js tools need wrapper
        assert!(needs_shell_wrapper("npx"));
        assert!(needs_shell_wrapper("npm"));
        assert!(needs_shell_wrapper("node"));
        assert!(needs_shell_wrapper("pnpm"));

        // Known Python tools need wrapper
        assert!(needs_shell_wrapper("uvx"));
        assert!(needs_shell_wrapper("uv"));
        assert!(needs_shell_wrapper("pip"));

        // Unknown commands don't need wrapper
        assert!(!needs_shell_wrapper("custom_binary"));
    }

    #[test]
    #[cfg(not(windows))]
    fn test_needs_shell_wrapper_unix() {
        // On Unix, nothing needs shell wrapper
        assert!(!needs_shell_wrapper("npx"));
        assert!(!needs_shell_wrapper("script.sh"));
        assert!(!needs_shell_wrapper("python"));
    }

    #[test]
    #[cfg(windows)]
    fn test_prepare_command_windows() {
        // Test npx wrapping
        let (cmd, args) = prepare_command("npx", &vec!["-y".to_string(), "package".to_string()]);
        assert_eq!(cmd, "cmd.exe");
        assert_eq!(args, vec!["/C", "npx", "-y", "package"]);

        // Test .exe passthrough
        let (cmd, args) = prepare_command("program.exe", &vec!["arg1".to_string()]);
        assert_eq!(cmd, "program.exe");
        assert_eq!(args, vec!["arg1"]);

        // Test .ps1 wrapping
        let (cmd, args) = prepare_command("script.ps1", &vec!["arg1".to_string()]);
        assert_eq!(cmd, "powershell.exe");
        assert!(args[0] == "-ExecutionPolicy");
        assert!(args[2] == "-File");
        assert!(args[3] == "script.ps1");
    }

    #[test]
    #[cfg(not(windows))]
    fn test_prepare_command_unix() {
        // On Unix, commands pass through unchanged
        let (cmd, args) = prepare_command("npx", &vec!["-y".to_string(), "package".to_string()]);
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "package"]);
    }
}
