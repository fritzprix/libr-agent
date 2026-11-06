/// Utility functions for cross-platform command execution
///
/// Windows requires special handling for .ps1 files (invoked via powershell.exe)
/// and resolving bare script names (.cmd) like npx, npm, pnpm, yarn.
///
/// Note: We NEVER wrap commands with `cmd.exe /C` because cmd.exe re-parses the
/// command line and interprets shell metacharacters (&, |, <, >, ^), leading to
/// command injection vulnerabilities (CVE-2024-24576). Instead, .bat and .cmd files
/// are executed directly via Rust's process spawner, which applies secure batch argument
/// escaping.
#[cfg(windows)]
use std::path::Path;

/// Determines if a command is a PowerShell script on Windows
#[cfg_attr(not(windows), allow(dead_code))]
fn is_powershell_script(command: &str) -> bool {
    #[cfg(windows)]
    {
        command.ends_with(".ps1")
    }
    #[cfg(not(windows))]
    {
        let _ = command;
        false
    }
}

/// Prepares a command for execution across platforms.
///
/// On Windows:
/// - `.ps1` files are launched via `powershell.exe -ExecutionPolicy Bypass -File <command> <args...>`
/// - Known Node.js/package manager batch shims (`npx`, `npm`, `pnpm`, `yarn`) without an extension
///   are resolved to their `.cmd` extension (`npx.cmd`, etc.) so Rust's `std::process::Command`
///   recognizes them as batch files and invokes them with Rust's hardened batch escaping logic
///   rather than unescaped `cmd.exe /C`.
/// - If a file path without extension has an existing `.cmd` or `.bat` file on disk, that extension
///   is appended.
/// - Native executables (`.exe`, `node`, `python`, `uv`, etc.) and `.cmd`/`.bat` scripts are
///   passed directly to `Command::new()`.
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

        let path = Path::new(command);
        let basename = path.file_name().and_then(|n| n.to_str()).unwrap_or(command);

        if path.extension().is_none() {
            if path.with_extension("cmd").is_file() {
                return (
                    path.with_extension("cmd").to_string_lossy().to_string(),
                    args.to_vec(),
                );
            }
            if path.with_extension("bat").is_file() {
                return (
                    path.with_extension("bat").to_string_lossy().to_string(),
                    args.to_vec(),
                );
            }
            if matches!(basename, "npx" | "npm" | "pnpm" | "yarn") {
                return (format!("{}.cmd", command), args.to_vec());
            }
        }
    }

    // Unix/Linux or direct Windows command (.exe, .cmd, .bat, etc.)
    (command.to_string(), args.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_prepare_command_windows() {
        // Test npx resolves to npx.cmd (never cmd.exe)
        let (cmd, args) = prepare_command("npx", &["-y".to_string(), "package".to_string()]);
        assert_eq!(cmd, "npx.cmd");
        assert_eq!(args, vec!["-y", "package"]);

        let (cmd, args) = prepare_command("npm", &["run".to_string()]);
        assert_eq!(cmd, "npm.cmd");
        assert_eq!(args, vec!["run"]);

        // Test .exe passthrough
        let (cmd, args) = prepare_command("program.exe", &["arg1".to_string()]);
        assert_eq!(cmd, "program.exe");
        assert_eq!(args, vec!["arg1"]);

        // Test .bat / .cmd passthrough (never cmd.exe)
        let (cmd, args) = prepare_command("script.cmd", &["arg1".to_string()]);
        assert_eq!(cmd, "script.cmd");
        assert_eq!(args, vec!["arg1"]);

        let (cmd, args) = prepare_command("script.bat", &["arg1".to_string()]);
        assert_eq!(cmd, "script.bat");
        assert_eq!(args, vec!["arg1"]);

        // Test native tools (.exe) passthrough
        let (cmd, args) = prepare_command("node", &["server.js".to_string()]);
        assert_eq!(cmd, "node");
        assert_eq!(args, vec!["server.js"]);

        let (cmd, args) = prepare_command("python", &["-m".to_string(), "mcp".to_string()]);
        assert_eq!(cmd, "python");
        assert_eq!(args, vec!["-m", "mcp"]);

        // Test .ps1 wrapping via powershell.exe -File
        let (cmd, args) = prepare_command("script.ps1", &["arg1".to_string()]);
        assert_eq!(cmd, "powershell.exe");
        assert_eq!(args[0], "-ExecutionPolicy");
        assert_eq!(args[1], "Bypass");
        assert_eq!(args[2], "-File");
        assert_eq!(args[3], "script.ps1");
        assert_eq!(args[4], "arg1");
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_cmd_metacharacters_argument_no_injection() {
        let temp_dir = std::env::temp_dir();
        let bat_path = temp_dir.join("libragent_test_metachars.bat");
        let canary_path = temp_dir.join("libragent_test_canary.txt");
        let _ = std::fs::remove_file(&canary_path);

        // Batch file that echoes its first argument to stdout
        let script = "@echo off\r\necho ARG1=%~1\r\n";
        std::fs::write(&bat_path, script).expect("write test bat");

        // Argument containing cmd metacharacters that would execute a command if parsed by cmd.exe /C:
        let canary_str = canary_path.to_str().unwrap();
        let malicious_arg = format!("safe_arg & echo INJECTED > \"{}\"", canary_str);

        let (final_cmd, final_args) =
            prepare_command(bat_path.to_str().unwrap(), &[malicious_arg.clone()]);

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

        // The argument must have arrived as a single literal argument
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("ARG1=safe_arg & echo INJECTED >"),
            "Argument must arrive intact: {}",
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
        let (cmd, args) = prepare_command("npx", &["-y".to_string(), "package".to_string()]);
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "package"]);
    }
}
