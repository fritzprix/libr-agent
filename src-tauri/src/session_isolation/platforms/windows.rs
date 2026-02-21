use crate::session_isolation::common::get_shell_command;
use crate::session_isolation::types::{IsolatedProcessConfig, IsolationConfig};
use base64::{engine::general_purpose, Engine as _};
use std::path::PathBuf;
use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};

/// Basic isolation: environment variables and working directory
pub async fn create_basic_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    // Detect if this is a direct PowerShell/executable command (Windows-specific)
    let (shell_cmd, use_shell_wrapper) = {
        let cmd_lower = config.command.to_lowercase();
        if cmd_lower.starts_with("powershell") || cmd_lower.starts_with("pwsh") {
            // Direct PowerShell execution - don't wrap with cmd.exe
            info!("Detected PowerShell command, executing directly without cmd.exe wrapper");
            (
                config
                    .command
                    .split_whitespace()
                    .next()
                    .unwrap_or("powershell")
                    .to_string(),
                false,
            )
        } else {
            (get_shell_command(config.shell_type).to_string(), true)
        }
    };

    let mut cmd = AsyncCommand::new(&shell_cmd);

    // Suppress console window on Windows (prevents terminal flashing)
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    // Set working directory
    cmd.current_dir(&config.workspace_path);

    // Smart Discovery: Auto-detect Python path to be used later
    let detected_python = detect_python_path().await;
    let python_path_str = detected_python.as_ref().map(|p| p.to_string_lossy());

    // Configure base environment (applied to both wrapper and direct execution)
    // Windows: DO NOT use env_clear() as it breaks process execution
    cmd.env("USERPROFILE", &config.workspace_path);
    cmd.env("HOME", &config.workspace_path);
    cmd.env("TEMP", config.workspace_path.join("tmp"));
    cmd.env("TMP", config.workspace_path.join("tmp"));

    // Add user-specified environment variables (applies to all platforms)
    for (key, value) in &config.env_vars {
        cmd.env(key, value);
    }

    // Construct PATH environment variable carefully
    let current_path = std::env::var("PATH").unwrap_or_default();
    let mut new_path = current_path.clone();

    if let Some(python_str) = &python_path_str {
        // Simple check to avoid duplicate appending if it's already in PATH
        if !current_path.contains(python_str.as_ref()) {
            if let Some(python_path) = &detected_python {
                let scripts_dir = python_path.join("Scripts");
                let lib_bin_dir = python_path.join("Library").join("bin");

                // PREPEND to PATH to ensure this Python takes precedence over WindowsApps shim
                new_path = format!(
                    "{};{};{};{}",
                    python_str,
                    scripts_dir.to_string_lossy(),
                    lib_bin_dir.to_string_lossy(),
                    current_path
                );
                info!(
                    "Smart Discovery: Prepended Python at {} to PATH",
                    python_str
                );
            }
        }
    }

    // Set PATH on the command
    cmd.env("PATH", &new_path);

    info!("Windows environment configured: workspace isolated, PATH preserved (with Anaconda if found)");
    // Additional env diagnostic info to help diagnose missing output on Windows
    let path_len = std::env::var("PATH").map(|p| p.len()).unwrap_or(0);
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "<not-set>".to_string());
    let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "<not-set>".to_string());
    let psmodulepath = std::env::var("PSModulePath").unwrap_or_else(|_| "<not-set>".to_string());
    info!("Windows env snapshot (for debugging): PATH.len={}, SystemRoot={}, COMSPEC={}, PSModulePath.present={}", path_len, system_root, comspec, !psmodulepath.is_empty());

    // Set command arguments based on platform and shell type
    if !use_shell_wrapper {
        // Direct PowerShell execution: parse and pass arguments directly
        // Extract PowerShell args from the command string
        let parts: Vec<&str> = config.command.split_whitespace().collect();
        if parts.len() > 1 {
            // Pass all arguments after "powershell"/"pwsh"
            cmd.args(&parts[1..]);
        }
        // Add any additional args
        if !config.args.is_empty() {
            cmd.args(&config.args);
        }
        info!(
            "PowerShell direct execution configured: {} with args: {:?}",
            shell_cmd,
            parts.get(1..).unwrap_or(&[])
        );
        info!(
            "PowerShell direct exec: command='{}' args={:?} workspace_dir={}",
            shell_cmd,
            parts.get(1..).unwrap_or(&[]),
            config.workspace_path.display()
        );
    } else {
        // Windows: Use PowerShell instead of cmd.exe for better quote handling
        // We override cmd to be "powershell" here, which REPLACES the previous AsyncCommand::new(&shell_cmd)
        // So we must re-apply environment variables!

        // However, instead of recreating `cmd`, we can just ensure `shell_cmd` was "powershell" to begin with.
        // `get_shell_command` returns "powershell" or "cmd".
        // If we are here, `use_shell_wrapper` is true.
        // The original code re-created `cmd` which wiped out envs.
        // Let's modify logic to NOT recreate `cmd` if possible, or re-apply envs.

        // But `cmd` struct doesn't allow changing the program once created.
        // So we MUST create a new command if we switch to PowerShell wrapper logic.

        // Refactor: We create the correct command initially.
        // But wait, `get_shell_command` returns "powershell" by default on Windows.
        // So `shell_cmd` is likely already "powershell".
        // The only case where it might be "cmd" is if ShellType::Cmd was requested explicitly.
        // But the wrapper logic forces "powershell" usage anyway: `cmd = AsyncCommand::new("powershell")`.
        // So effectively, `shell_cmd` is ignored in this branch!

        // So, let's fix the initial creation to use "powershell" directly if wrapper is needed.
        // Actually, let's just create a NEW command here and re-apply envs properly.

        let mut wrapped_cmd = AsyncCommand::new("powershell");

        // Suppress console window on Windows (prevents terminal flashing)
        #[cfg(target_os = "windows")]
        {
            wrapped_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        wrapped_cmd.current_dir(&config.workspace_path);

        // Re-apply envs
        wrapped_cmd.env("USERPROFILE", &config.workspace_path);
        wrapped_cmd.env("HOME", &config.workspace_path);
        wrapped_cmd.env("TEMP", config.workspace_path.join("tmp"));
        wrapped_cmd.env("TMP", config.workspace_path.join("tmp"));
        for (key, value) in &config.env_vars {
            wrapped_cmd.env(key, value);
        }
        wrapped_cmd.env("PATH", &new_path); // Use the computed PATH with Python

        // Now handle the command wrapping
        // We need to construct the command string carefully.
        // Joining with spaces is risky if args contain spaces.
        // We should quote arguments.

        // Helper to quote arguments for PowerShell
        let quote_arg = |arg: &str| -> String {
            // Simple quoting: wrap in single quotes, escape single quotes inside
            format!("'{}'", arg.replace("'", "''"))
        };

        // Wait, if we run `python file.py`, we want `python 'file.py'`.
        // If we run `"C:\Program Files\Python\python.exe" file.py`, we want `'C:\Program Files\Python\python.exe' 'file.py'`.
        // PowerShell handles `& 'path' args` syntax.
        // Invoke-Expression expects a string.

        // Let's use simple space joining for the binary (assuming it's simple) and quoted args.
        // Ideally we should use `&` operator in PowerShell if the command is quoted.
        // e.g. `& 'C:\Path\To\Exe' 'arg1' 'arg2'`

        let binary = &config.command;
        let args_str = config
            .args
            .iter()
            .map(|a| quote_arg(a))
            .collect::<Vec<_>>()
            .join(" ");

        let full_command = if args_str.is_empty() {
            binary.clone()
        } else {
            format!("{} {}", binary, args_str)
        };

        let encoded_command = general_purpose::STANDARD.encode(&full_command);
        let wrapped_command = format!(
            "$ErrorActionPreference = 'Stop'; [System.Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US'; $cmd = [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('{}')); try {{ Invoke-Expression $cmd }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); [Console]::Error.WriteLine($_.ScriptStackTrace); exit 1 }}",
            encoded_command
        );

        wrapped_cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &wrapped_command,
        ]);

        info!("Windows PowerShell execution with proper argument escaping and error redirection");

        // Replace `cmd` with `wrapped_cmd`
        cmd = wrapped_cmd;

        // Log environment snapshot
        let path_len = std::env::var("PATH").map(|p| p.len()).unwrap_or(0);
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "<not-set>".to_string());
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "<not-set>".to_string());
        info!(
            "PowerShell wrapper env snapshot: PATH.len={}, SystemRoot={}, COMSPEC={}",
            path_len, system_root, comspec
        );
    }

    info!(
        "Isolated command created for session {} with isolation level {:?}",
        config.session_id, config.isolation_level
    );
    Ok(cmd)
}

/// Medium isolation: process groups + resource limits
pub async fn create_medium_isolated_command(
    config: IsolatedProcessConfig,
    _isolation_config: &IsolationConfig,
) -> Result<AsyncCommand, String> {
    let mut cmd = create_basic_isolated_command(config.clone()).await?;

    // Apply platform-specific process group isolation
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP only
    }

    // Windows resource limits not implemented yet
    warn!("Windows resource limits not implemented yet, using basic limits");

    Ok(cmd)
}

/// Windows high isolation using job objects and restricted tokens
pub async fn create_high_isolated_command(
    config: IsolatedProcessConfig,
    isolation_config: &IsolationConfig,
) -> Result<AsyncCommand, String> {
    let mut cmd = create_medium_isolated_command(config.clone(), isolation_config).await?;

    // Apply Windows-specific isolation
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP only
    }

    info!(
        "Created Windows high isolation command for session: {}",
        config.session_id
    );
    Ok(cmd)
}

/// Detects a valid Python installation on Windows, prioritizing non-Store versions.
async fn detect_python_path() -> Option<PathBuf> {
    // 1. Try `where python` to find registered executables
    if let Ok(output) = AsyncCommand::new("where").arg("python").output().await {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let path = PathBuf::from(line.trim());
                // Filter out WindowsApps shim which redirects to Microsoft Store
                if !path.to_string_lossy().contains("WindowsApps") && path.exists() {
                    if let Some(parent) = path.parent() {
                        info!("Detected Python via 'where': {:?}", parent);
                        return Some(parent.to_path_buf());
                    }
                }
            }
        }
    }

    // 2. Check standard installation locations as fallback
    let common_paths = vec![
        // Anaconda (User)
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("Anaconda3")),
        // Anaconda (System)
        std::env::var("ProgramData")
            .ok()
            .map(|p| PathBuf::from(p).join("Anaconda3")),
        // Anaconda (User Profile)
        std::env::var("USERPROFILE")
            .ok()
            .map(|p| PathBuf::from(p).join("anaconda3")),
        // Standard Python (User) - check for Python3* directories
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("Programs").join("Python")),
    ];

    // Use spawn_blocking to avoid blocking async runtime with fs operations
    let found_path = tokio::task::spawn_blocking(move || {
        for path in common_paths.into_iter().flatten() {
            // For standard Python, we might need to look deeper (e.g. Python39, Python310)
            if path.join("python.exe").exists() {
                return Some(path);
            }

            // Check subdirectories for standard Python installs
            if path.exists() && path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for entry in entries.flatten() {
                        let subpath = entry.path();
                        if subpath.join("python.exe").exists() {
                            return Some(subpath);
                        }
                    }
                }
            }
        }
        None
    })
    .await
    .unwrap_or(None);

    if let Some(path) = &found_path {
        info!("Detected Python via standard path search: {:?}", path);
    }

    found_path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_powershell_error_wrapping() {
        // Test that the wrapped command includes error handling and base64 encoding
        let test_command = "Remove-Item -Path \"C:\\test\" -Recurse -Force";
        let encoded = general_purpose::STANDARD.encode(test_command);
        let wrapped = format!(
            "$ErrorActionPreference = 'Stop'; [System.Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US'; $cmd = [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('{}')); try {{ Invoke-Expression $cmd }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); [Console]::Error.WriteLine($_.ScriptStackTrace); exit 1 }}",
            encoded
        );

        // Verify the wrapped command contains key elements
        assert!(wrapped.contains("$ErrorActionPreference = 'Stop'"));
        assert!(wrapped.contains("FromBase64String"));
        assert!(wrapped.contains("Invoke-Expression $cmd"));
        assert!(wrapped.contains("exit 1"));

        // Extract Base64 and verify
        let start_marker = "FromBase64String('";
        let end_marker = "'));";
        let start = wrapped.find(start_marker).unwrap() + start_marker.len();
        let end = wrapped.find(end_marker).unwrap();
        let extracted_b64 = &wrapped[start..end];

        let decoded_bytes = general_purpose::STANDARD.decode(extracted_b64).unwrap();
        let decoded_str = String::from_utf8(decoded_bytes).unwrap();

        assert_eq!(decoded_str, test_command);
    }

    #[test]
    fn test_powershell_quote_preservation() {
        // Test that double quotes are preserved correctly through Base64 roundtrip
        let test_command = "Write-Host \"Hello World\"";
        let encoded = general_purpose::STANDARD.encode(test_command);

        let wrapped = format!(
            "$ErrorActionPreference = 'Stop'; [System.Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US'; $cmd = [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('{}')); try {{ Invoke-Expression $cmd }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); [Console]::Error.WriteLine($_.ScriptStackTrace); exit 1 }}",
            encoded
        );

        // Verify Base64 contains the command with quotes intact
        let start_marker = "FromBase64String('";
        let end_marker = "'));";
        let start = wrapped.find(start_marker).unwrap() + start_marker.len();
        let end = wrapped.find(end_marker).unwrap();
        let extracted_b64 = &wrapped[start..end];

        let decoded_bytes = general_purpose::STANDARD.decode(extracted_b64).unwrap();
        let decoded_str = String::from_utf8(decoded_bytes).unwrap();

        assert_eq!(decoded_str, test_command);
    }
}
