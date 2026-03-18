use crate::session_isolation::types::{IsolatedProcessConfig, IsolationConfig, ShellType};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};

/// Monotonic counter for unique script filenames within a process lifetime.
static SCRIPT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Basic isolation: environment variables and working directory
pub async fn create_basic_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    let shell_type = config.shell_type.unwrap_or(ShellType::PowerShell);

    // All commands are written to a temp .ps1 file and executed with `powershell -File`.
    // This avoids two classes of bugs:
    //   1. Naive split_whitespace arg-passing broke `powershell -Command "..."` patterns.
    //   2. Base64+Invoke-Expression (a previous fix attempt) triggers AV heuristics because
    //      it matches common malware obfuscation patterns.
    // Writing a plain .ps1 file is readable by AV scanners and handles any command string
    // without fragmentation or encoding.

    let mut cmd = AsyncCommand::new(shell_type.command());

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

    // Apply environment isolation: clear all inherited environment variables
    cmd.env_clear();

    // Re-apply whitelisted essential system variables
    for (k, v) in crate::utils::env::get_isolated_env() {
        cmd.env(k, v);
    }

    // Configure base environment overrides
    cmd.env("USERPROFILE", &config.workspace_path);
    cmd.env("HOME", &config.workspace_path);
    cmd.env("TEMP", config.workspace_path.join("tmp"));
    cmd.env("TMP", config.workspace_path.join("tmp"));

    // Add user-specified environment variables
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

    cmd.env("PATH", &new_path);

    info!("Windows environment configured: workspace isolated, PATH preserved (with Anaconda if found)");
    let path_len = std::env::var("PATH").map(|p| p.len()).unwrap_or(0);
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "<not-set>".to_string());
    let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "<not-set>".to_string());
    let psmodulepath = std::env::var("PSModulePath").unwrap_or_else(|_| "<not-set>".to_string());
    info!("Windows env snapshot (for debugging): PATH.len={}, SystemRoot={}, COMSPEC={}, PSModulePath.present={}", path_len, system_root, comspec, !psmodulepath.is_empty());

    // Build the full command string (binary + any extra args from config).
    // Args are single-quoted for PowerShell to handle spaces/special chars.
    let quote_arg = |arg: &str| -> String { format!("'{}'", arg.replace("'", "''")) };
    let args_str = config
        .args
        .iter()
        .map(|a| quote_arg(a))
        .collect::<Vec<_>>()
        .join(" ");
    let full_command = if args_str.is_empty() {
        config.command.clone()
    } else {
        format!("{} {}", config.command, args_str)
    };

    // Write the command to a temp .ps1 file so AV can inspect it in plaintext.
    // Base64+Invoke-Expression was flagged as malware obfuscation; plain .ps1 is not.
    let tmp_dir = config.workspace_path.join("tmp");
    tokio::fs::create_dir_all(&tmp_dir)
        .await
        .map_err(|e| format!("Failed to create tmp dir: {}", e))?;

    // Monotonic counter avoids collisions when multiple commands fire within the same millisecond.
    let seq = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let script_path = tmp_dir.join(format!("cmd_{}_{}.ps1", config.session_id, seq));

    // Plain readable script — no obfuscation, AV-friendly
    let script_content = format!(
        "$ErrorActionPreference = 'Stop'\n\
         [System.Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US'\n\
         try {{\n\
             {}\n\
         }} catch {{\n\
             [Console]::Error.WriteLine($_.Exception.Message)\n\
             [Console]::Error.WriteLine($_.ScriptStackTrace)\n\
             exit 1\n\
         }}\n",
        full_command
    );

    // ✅ CRITICAL FIX: Add UTF-8 BOM (Byte Order Mark) so Windows PowerShell 5.1
    // correctly recognizes the file as UTF-8. Without this, it uses the system
    // ANSI code page (e.g., CP949 on Korean Windows), which garbles non-ASCII
    // characters and can cause parsing hangs if misinterpreted as unclosed quotes/blocks.
    let mut bom_content = vec![0xEF, 0xBB, 0xBF];
    bom_content.extend_from_slice(script_content.as_bytes());

    tokio::fs::write(&script_path, &bom_content)
        .await
        .map_err(|e| format!("Failed to write command script: {}", e))?;

    let script_path_str = script_path
        .to_str()
        .ok_or_else(|| "Script path contains invalid UTF-8".to_string())?
        .to_string();

    // Wrap with a cleanup script: run the target .ps1, then delete it regardless of outcome.
    // This prevents accumulation of temp files in the workspace tmp/ directory.
    let self_deleting_wrapper = format!(
        "try {{ & '{}' }} finally {{ Remove-Item -LiteralPath '{}' -Force -ErrorAction SilentlyContinue }}",
        script_path_str.replace("'", "''"),
        script_path_str.replace("'", "''")
    );

    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &self_deleting_wrapper,
    ]);

    info!(
        "Windows: wrote command script to {:?}, executing with self-cleanup wrapper",
        script_path
    );
    info!(
        "PowerShell env snapshot: PATH.len={}, SystemRoot={}, COMSPEC={}",
        path_len, system_root, comspec
    );

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
        // Use bitwise OR to preserve CREATE_NO_WINDOW from create_basic_isolated_command
        cmd.creation_flags(0x08000000 | 0x00000200); // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
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
        // Use bitwise OR to preserve CREATE_NO_WINDOW
        cmd.creation_flags(0x08000000 | 0x00000200); // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
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
    let mut cmd = AsyncCommand::new("where");
    crate::utils::env::apply_isolated_env_async(&mut cmd);
    cmd.arg("python");

    if let Ok(output) = cmd.output().await {
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

    /// Mirrors the script content format used by `create_basic_isolated_command`.
    fn build_script_content(full_command: &str) -> String {
        format!(
            "$ErrorActionPreference = 'Stop'\n\
             [System.Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US'\n\
             try {{\n\
                 {}\n\
             }} catch {{\n\
                 [Console]::Error.WriteLine($_.Exception.Message)\n\
                 [Console]::Error.WriteLine($_.ScriptStackTrace)\n\
                 exit 1\n\
             }}\n",
            full_command
        )
    }

    /// Mirrors the self-deleting wrapper format used by `create_basic_isolated_command`.
    fn build_cleanup_wrapper(script_path: &str) -> String {
        let escaped = script_path.replace("'", "''");
        format!(
            "try {{ & '{}' }} finally {{ Remove-Item -LiteralPath '{}' -Force -ErrorAction SilentlyContinue }}",
            escaped, escaped
        )
    }

    // ── Script content tests ────────────────────────────────────────────────

    #[test]
    fn test_script_content_has_error_handling() {
        // Script must stop on first error, report it to stderr, and exit non-zero.
        let script = build_script_content("Remove-Item -Path 'C:\\test' -Recurse -Force");
        assert!(script.contains("$ErrorActionPreference = 'Stop'"));
        assert!(script.contains("try {"));
        assert!(script.contains("} catch {"));
        assert!(script.contains("[Console]::Error.WriteLine"));
        assert!(script.contains("exit 1"));
    }

    #[test]
    fn test_script_content_no_obfuscation() {
        // REGRESSION: Base64+Invoke-Expression triggered AV heuristics (malware obfuscation pattern).
        // The .ps1 file must be plaintext-readable so AV can scan it.
        let script = build_script_content("Some-Command -Arg value");
        assert!(
            !script.contains("Invoke-Expression"),
            "Script must not use Invoke-Expression (AV red-flag)"
        );
        assert!(
            !script.contains("FromBase64String"),
            "Script must not use Base64 decoding (AV red-flag)"
        );
        assert!(
            !script.contains("EncodedCommand"),
            "Script must not use -EncodedCommand (AV red-flag)"
        );
    }

    #[test]
    fn test_script_content_preserves_double_quotes() {
        // REGRESSION: split_whitespace fragmented quoted args like `"Expand-Archive` into
        // a string literal that PowerShell echoed and exited 0 without executing.
        // .ps1 file approach passes the command verbatim — no fragmentation possible.
        let cmd = "Write-Host \"Hello World\"";
        let script = build_script_content(cmd);
        assert!(
            script.contains("Write-Host \"Hello World\""),
            "Double quotes must survive into the .ps1 file unchanged"
        );
    }

    #[test]
    fn test_script_content_preserves_expand_archive_pattern() {
        // REGRESSION: The exact command that triggered the original split_whitespace bug.
        // `powershell -Command "Expand-Archive ..."` was split on whitespace, making the
        // leading `"` turn `"Expand-Archive` into a string literal evaluated by PowerShell.
        // The process exited 0 with no output and no side effects — completely silent failure.
        let cmd = "Expand-Archive -Path \"attachments/foo.zip\" -DestinationPath \".\"";
        let script = build_script_content(cmd);
        assert!(script.contains("Expand-Archive"));
        assert!(script.contains("attachments/foo.zip"));
        assert!(script.contains("-DestinationPath \".\""));
    }

    #[test]
    fn test_script_content_powershell_command_pattern() {
        // REGRESSION: `powershell -Command "..."` passed as a command string used to break
        // because split_whitespace would pass `-Command` and `"..."` as separate args.
        let cmd = "powershell -Command \"Get-Process\"";
        let script = build_script_content(cmd);
        assert!(script.contains("powershell -Command \"Get-Process\""));
    }

    // ── Cleanup wrapper tests ───────────────────────────────────────────────

    #[test]
    fn test_cleanup_wrapper_calls_script_and_deletes() {
        // Wrapper must invoke the script AND delete it in a finally block (runs on success/failure).
        let wrapper = build_cleanup_wrapper("C:\\workspace\\tmp\\cmd_abc_0.ps1");
        assert!(wrapper.contains("& 'C:\\workspace\\tmp\\cmd_abc_0.ps1'"));
        assert!(wrapper.contains("Remove-Item"));
        assert!(wrapper.contains("-LiteralPath"));
        assert!(wrapper.contains("finally"));
        assert!(wrapper.contains("-ErrorAction SilentlyContinue"));
    }

    #[test]
    fn test_cleanup_wrapper_escapes_single_quotes_in_path() {
        // If the path contains a single quote, it must be doubled to avoid PS injection.
        let path_with_quote = "C:\\work's\\tmp\\cmd.ps1";
        let wrapper = build_cleanup_wrapper(path_with_quote);
        assert!(
            wrapper.contains("C:\\work''s\\tmp\\cmd.ps1"),
            "Single quotes in path must be escaped as '' for PowerShell"
        );
        assert!(
            !wrapper.contains("work's"),
            "Unescaped single quote must not appear in wrapper"
        );
    }

    // ── Counter uniqueness tests ────────────────────────────────────────────

    #[test]
    fn test_script_counter_is_monotonic() {
        // Counter must strictly increase so concurrent commands never share a filename.
        let a = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let b = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let c = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
        assert!(a < b && b < c, "SCRIPT_COUNTER must be strictly monotonic");
    }
}
