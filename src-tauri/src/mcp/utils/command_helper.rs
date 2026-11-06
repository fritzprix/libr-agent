/// Utility functions for cross-platform command execution
///
/// Windows requires special handling for .ps1 files (invoked via powershell.exe)
/// and resolving bare command names against PATH × PATHEXT (respecting PATHEXT order,
/// e.g. .exe before .cmd / .bat) so that batch scripts (.cmd/.bat) and native binaries
/// (.exe) are spawned directly with secure argument escaping without cmd.exe shell wrapper.
///
/// Note: We NEVER wrap commands with `cmd.exe /C` because cmd.exe re-parses the
/// command line and interprets shell metacharacters (&, |, <, >, ^), leading to
/// command injection vulnerabilities (CVE-2024-24576). Instead, .bat and .cmd files
/// are executed directly via Rust's process spawner, which applies secure batch argument
/// escaping.
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Determines if a command is a PowerShell script on Windows (case-insensitive)
#[cfg_attr(not(windows), allow(dead_code))]
fn is_powershell_script(command: &str) -> bool {
    #[cfg(windows)]
    {
        command.to_ascii_lowercase().ends_with(".ps1")
    }
    #[cfg(not(windows))]
    {
        let _ = command;
        false
    }
}

/// Returns the list of executable extensions from PATHEXT, or standard defaults
#[cfg_attr(not(windows), allow(dead_code))]
fn get_pathext_extensions() -> Vec<String> {
    let pathext = std::env::var("PATHEXT").unwrap_or_default();
    if pathext.is_empty() {
        vec![
            ".com".to_string(),
            ".exe".to_string(),
            ".bat".to_string(),
            ".cmd".to_string(),
        ]
    } else {
        pathext
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                let s = s.to_ascii_lowercase();
                if s.starts_with('.') {
                    s
                } else {
                    format!(".{}", s)
                }
            })
            .collect()
    }
}

/// Helper to search a list of directories for a command matching candidate extensions
#[cfg_attr(not(windows), allow(dead_code))]
fn resolve_command_in_dirs(
    command: &str,
    dirs: &[&Path],
    extensions: &[String],
) -> Option<PathBuf> {
    for dir in dirs {
        for ext in extensions {
            let candidate = dir.join(format!("{}{}", command, ext));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Resolves a command on Windows against the child process PATH and PATHEXT.
///
/// - If `command` already has an extension, returns it as-is.
/// - If `command` contains path separators (`/` or `\`), checks candidate extensions at that path.
/// - If `command` is a bare name, searches child PATH (falling back to process PATH) directories
///   in order, checking extensions in PATHEXT order (.exe prioritized before .cmd / .bat).
/// - If not found, falls back to the bare command name.
#[cfg(windows)]
fn resolve_windows_command(command: &str, child_path: Option<&OsStr>) -> String {
    let path = Path::new(command);

    if path.extension().is_some() {
        return command.to_string();
    }

    let extensions = get_pathext_extensions();

    if command.contains('/') || command.contains('\\') {
        for ext in &extensions {
            let candidate = PathBuf::from(format!("{}{}", command, ext));
            if candidate.is_file() {
                return candidate.to_string_lossy().to_string();
            }
        }
        return command.to_string();
    }

    let path_val = match child_path {
        Some(p) => Some(p.to_os_string()),
        None => std::env::var_os("PATH"),
    };

    if let Some(path_var) = path_val {
        let dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
        let dir_refs: Vec<&Path> = dirs.iter().map(|p| p.as_path()).collect();
        if let Some(found) = resolve_command_in_dirs(command, &dir_refs, &extensions) {
            return found.to_string_lossy().to_string();
        }
    }

    command.to_string()
}

/// Prepares a command for execution across platforms.
///
/// On Windows:
/// - `.ps1` files are launched via `powershell.exe -ExecutionPolicy Bypass -File <command> <args...>`
/// - Bare commands (without extension) are resolved against the child's `PATH` × `PATHEXT`
///   (respecting PATHEXT order, e.g. .exe before .cmd/.bat) so that Rust's `std::process::Command`
///   recognizes batch files (`.cmd`, `.bat`) and applies its hardened batch escaping logic rather
///   than unescaped `cmd.exe /C`. If a `.exe` shim is present (e.g., Volta, Scoop), it is prioritized
///   and executed directly.
/// - Direct `.exe`, `.cmd`, `.bat` commands are passed through directly to `Command::new()`.
///
/// # Arguments
/// * `command` - The command to execute
/// * `args` - Command arguments
/// * `child_path` - Optional child process PATH to search for bare commands (falls back to process PATH if None)
///
/// # Returns
/// Tuple of (final_command, final_args) ready for Command::new()
pub fn prepare_command(
    command: &str,
    args: &[String],
    child_path: Option<&OsStr>,
) -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        if is_powershell_script(command) {
            let mut new_args = vec![
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                command.to_string(),
            ];
            new_args.extend(args.iter().cloned());
            return ("powershell.exe".to_string(), new_args);
        }

        let resolved = resolve_windows_command(command, child_path);
        return (resolved, args.to_vec());
    }

    #[cfg(not(windows))]
    {
        let _ = child_path;
        (command.to_string(), args.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_command_in_dirs() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let temp_path = temp_dir.path();

        let exe_file = temp_path.join("tool.exe");
        let cmd_file = temp_path.join("script.cmd");
        let both_exe = temp_path.join("multi.exe");
        let both_cmd = temp_path.join("multi.cmd");

        std::fs::write(&exe_file, b"").expect("write exe");
        std::fs::write(&cmd_file, b"").expect("write cmd");
        std::fs::write(&both_exe, b"").expect("write both exe");
        std::fs::write(&both_cmd, b"").expect("write both cmd");

        let extensions = vec![".exe".to_string(), ".cmd".to_string(), ".bat".to_string()];

        // Finds .exe
        let resolved = resolve_command_in_dirs("tool", &[temp_path], &extensions);
        assert_eq!(resolved, Some(exe_file));

        // Finds .cmd
        let resolved = resolve_command_in_dirs("script", &[temp_path], &extensions);
        assert_eq!(resolved, Some(cmd_file));

        // Prioritizes .exe over .cmd based on extensions order
        let resolved = resolve_command_in_dirs("multi", &[temp_path], &extensions);
        assert_eq!(resolved, Some(both_exe));

        // Returns None when not found
        let resolved = resolve_command_in_dirs("nonexistent", &[temp_path], &extensions);
        assert_eq!(resolved, None);
    }

    #[test]
    #[cfg(windows)]
    fn test_prepare_command_windows() {
        // Test .exe passthrough
        let (cmd, args) = prepare_command("program.exe", &["arg1".to_string()], None);
        assert_eq!(cmd, "program.exe");
        assert_eq!(args, vec!["arg1"]);

        // Test .bat / .cmd passthrough (never cmd.exe)
        let (cmd, args) = prepare_command("script.cmd", &["arg1".to_string()], None);
        assert_eq!(cmd, "script.cmd");
        assert_eq!(args, vec!["arg1"]);

        let (cmd, args) = prepare_command("script.bat", &["arg1".to_string()], None);
        assert_eq!(cmd, "script.bat");
        assert_eq!(args, vec!["arg1"]);

        // Test native tools (.exe) passthrough
        let (cmd, args) = prepare_command("node.exe", &["server.js".to_string()], None);
        assert_eq!(cmd, "node.exe");
        assert_eq!(args, vec!["server.js"]);

        let (cmd, args) =
            prepare_command("python.exe", &["-m".to_string(), "mcp".to_string()], None);
        assert_eq!(cmd, "python.exe");
        assert_eq!(args, vec!["-m", "mcp"]);

        // Test .ps1 / .PS1 wrapping via powershell.exe -File (case-insensitive, S1.1)
        let (cmd, args) = prepare_command("script.ps1", &["arg1".to_string()], None);
        assert_eq!(cmd, "powershell.exe");
        assert_eq!(
            args,
            vec!["-ExecutionPolicy", "Bypass", "-File", "script.ps1", "arg1"]
        );

        let (cmd, args) = prepare_command("SCRIPT.PS1", &["arg1".to_string()], None);
        assert_eq!(cmd, "powershell.exe");
        assert_eq!(
            args,
            vec!["-ExecutionPolicy", "Bypass", "-File", "SCRIPT.PS1", "arg1"]
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_prepare_command_windows_path_resolution() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let temp_path = temp_dir.path();

        // Create mock .exe and .cmd shims (G1.2 / G2.1)
        let exe_path = temp_path.join("mock_volta_shim.exe");
        let cmd_path = temp_path.join("mock_npm_shim.cmd");
        let both_exe = temp_path.join("mock_both_shim.exe");
        let both_cmd = temp_path.join("mock_both_shim.cmd");

        std::fs::write(&exe_path, b"").expect("create mock .exe");
        std::fs::write(&cmd_path, b"@echo off\r\n").expect("create mock .cmd");
        std::fs::write(&both_exe, b"").expect("create both .exe");
        std::fs::write(&both_cmd, b"@echo off\r\n").expect("create both .cmd");

        let test_path = temp_path.as_os_str();

        // Bare name resolving to .exe via child_path without mutating process env (G2.1, S2.1)
        let (resolved_exe, _) = prepare_command("mock_volta_shim", &[], Some(test_path));
        assert_eq!(
            Path::new(&resolved_exe),
            exe_path,
            "bare name should resolve to .exe shim in child PATH"
        );

        // Bare name resolving to .cmd via child_path
        let (resolved_cmd, _) = prepare_command("mock_npm_shim", &[], Some(test_path));
        assert_eq!(
            Path::new(&resolved_cmd),
            cmd_path,
            "bare name should resolve to .cmd shim in child PATH"
        );

        // When both .exe and .cmd exist, .exe must be prioritized (per PATHEXT order)
        let (resolved_both, _) = prepare_command("mock_both_shim", &[], Some(test_path));
        assert_eq!(
            Path::new(&resolved_both),
            both_exe,
            ".exe shim must be prioritized over .cmd shim in child PATH"
        );

        // Unmatched bare name falls back to original command name
        let (fallback, _) =
            prepare_command("nonexistent_bare_tool_libragent", &[], Some(test_path));
        assert_eq!(fallback, "nonexistent_bare_tool_libragent");
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_cmd_metacharacters_argument_no_injection() {
        let temp_dir = std::env::temp_dir();
        let bat_path = temp_dir.join("libragent_test_metachars.bat");
        let canary_path = temp_dir.join("libragent_test_canary.txt");
        let _ = std::fs::remove_file(&canary_path);

        // Batch file that echoes its first argument to stdout WITHOUT %~1 quote-stripping (G1.1)
        let script = "@echo off\r\necho ARG1=%1\r\n";
        std::fs::write(&bat_path, script).expect("write test bat");

        // Argument containing cmd metacharacters that would execute a command if parsed by cmd.exe /C:
        let canary_str = canary_path.to_str().unwrap();
        let malicious_arg = format!("safe_arg & echo INJECTED > \"{}\"", canary_str);

        let (final_cmd, final_args) =
            prepare_command(bat_path.to_str().unwrap(), &[malicious_arg.clone()], None);

        // final_cmd must NOT be cmd.exe
        assert_ne!(final_cmd, "cmd.exe");
        assert_eq!(final_cmd, bat_path.to_str().unwrap());
        assert_eq!(final_args, vec![malicious_arg]);

        // Execute the command using std::process::Command
        let output = std::process::Command::new(&final_cmd)
            .args(&final_args)
            .output()
            .expect("execute test bat");

        // The canary file must NOT have been created
        assert!(
            !canary_path.exists(),
            "Command injection occurred: canary file was created by cmd metacharacter!"
        );

        // The argument must have arrived intact with enclosing quotes
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("ARG1=\"safe_arg & echo INJECTED >"),
            "Argument must arrive intact with quotes: {}",
            stdout
        );

        // Cleanup
        let _ = std::fs::remove_file(&bat_path);
        let _ = std::fs::remove_file(&canary_path);
    }

    #[test]
    #[cfg(not(windows))]
    fn test_prepare_command_unix() {
        // On Unix, commands pass through unchanged
        let (cmd, args) = prepare_command("npx", &["-y".to_string(), "package".to_string()], None);
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "package"]);
    }
}
