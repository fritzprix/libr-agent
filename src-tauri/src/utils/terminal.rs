use std::path::Path;
use std::process::Command;

use super::platform::command_exists;

/// Construct the terminal launch command for the current platform
fn get_terminal_command(path: &Path) -> Result<(String, Vec<String>), String> {
    #[cfg(target_os = "windows")]
    {
        if !command_exists("cmd") {
            return Err("Required system command 'cmd' not found.".to_string());
        }

        // Normalize path to use backslashes for Windows cmd
        let path_str = path.to_string_lossy().replace("/", "\\");

        Ok((
            "cmd".to_string(),
            vec![
                "/c".to_string(),
                "start".to_string(),
                "Agent Workspace".to_string(),
                "/D".to_string(),
                path_str,
                "cmd".to_string(),
                "/k".to_string(),
            ],
        ))
    }

    #[cfg(target_os = "macos")]
    {
        if !command_exists("osascript") {
            return Err("Required system command 'osascript' not found. AppleScript support is required on macOS.".to_string());
        }

        let path_str = path.to_string_lossy();
        // Quote for shell: replace ' with '\'' and wrap in '
        let shell_quoted = format!("'{}'", path_str.replace("'", "'\\''"));
        // Escape for AppleScript string: replace \ with \\ and " with \"
        let script_cmd = format!("cd {}", shell_quoted);
        let applescript_escaped = script_cmd.replace("\\", "\\\\").replace("\"", "\\\"");

        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            applescript_escaped
        );

        Ok(("osascript".to_string(), vec!["-e".to_string(), script]))
    }

    #[cfg(target_os = "linux")]
    {
        let path_str = path.to_string_lossy().to_string();

        // Priority list of terminal emulators
        // Format: (executable, working_dir_flag, needs_cd_hack)
        let terminals = [
            ("gnome-terminal", Some("--working-directory"), false),
            ("konsole", Some("--workdir"), false),
            ("xfce4-terminal", Some("--working-directory"), false),
            ("x-terminal-emulator", None, true), // Fallback via Debian alternatives
            ("xterm", None, true),
        ];

        for (term, workdir_flag, needs_cd_hack) in terminals {
            if command_exists(term) {
                let mut args = Vec::new();

                if let Some(flag) = workdir_flag {
                    args.push(flag.to_string());
                    args.push(path_str.clone());
                } else if needs_cd_hack {
                    // xterm -e sh -c "cd 'path' && exec $SHELL"
                    args.push("-e".to_string());
                    args.push("sh".to_string());
                    args.push("-c".to_string());
                    // Escape single quotes for shell
                    let path_quoted = format!("'{}'", path_str.replace("'", "'\\''"));
                    args.push(format!("cd {} && exec $SHELL", path_quoted));
                }

                return Ok((term.to_string(), args));
            }
        }

        Err(format!(
            "No supported terminal emulator found (checked: gnome-terminal, konsole, xfce4-terminal, x-terminal-emulator, xterm). \
             Please install one of these or open a terminal manually at: {}",
            path.display()
        ))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Terminal launch not supported on this platform".to_string())
    }
}

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
    let (program, args) = get_terminal_command(path)?;

    Command::new(&program)
        .args(&args)
        .spawn()
        .map_err(|e| format!("Failed to spawn terminal '{}': {}", program, e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_path_normalization() {
        let path = PathBuf::from("C:/Users/test/workspace");
        let (prog, args) = get_terminal_command(&path).unwrap();

        assert_eq!(prog, "cmd");
        // Verify path in args has backslashes
        assert!(args.contains(&"C:\\Users\\test\\workspace".to_string()));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_get_terminal_command_with_special_chars() {
        // Paths with single quotes exercise the AppleScript shell-escaping logic
        let path = PathBuf::from("/Users/test/My Project 'Quoted' & $pecial");
        let (prog, args) = get_terminal_command(&path).unwrap();
        assert_eq!(prog, "osascript");
        assert!(!args.is_empty(), "osascript arguments should not be empty");
        let script = args.join(" ");
        assert!(
            script.contains("tell application \"Terminal\""),
            "should generate AppleScript"
        );
        assert!(script.contains("cd "), "script should contain a cd command");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_get_terminal_command_smoke() {
        // Returns a valid terminal command or a descriptive error when none is installed
        let path = PathBuf::from("/tmp/test workspace with spaces");
        let result = get_terminal_command(&path);
        match result {
            Ok((prog, args)) => {
                assert!(
                    !prog.is_empty(),
                    "terminal program name should not be empty"
                );
                assert!(!args.is_empty(), "arguments should not be empty");
                let joined = args.join(" ");
                let path_str = path.to_str().unwrap();
                assert!(
                    joined.contains(path_str) || joined.contains("cd "),
                    "args should reference the working directory directly or via cd, got: {}",
                    joined
                );
            }
            Err(msg) => {
                // Acceptable on headless CI without any terminal emulator installed
                assert!(
                    msg.contains("No supported terminal emulator found"),
                    "error should indicate no terminal was found, got: {}",
                    msg
                );
            }
        }
    }
}
