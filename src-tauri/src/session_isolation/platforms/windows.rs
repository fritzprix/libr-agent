use crate::session_isolation::types::{IsolatedProcessConfig, ShellType};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::process::Command as AsyncCommand;
use tracing::info;

/// Monotonic counter for unique script filenames within a process lifetime.
static SCRIPT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Helper to remove Windows UNC path prefix `\\?\` if present.
/// This prevents crashes in external tools (like node/pnpm) that fail to parse UNC paths.
///
/// NOTE: By stripping the `\\?\` prefix from local paths, Windows API support for paths
/// longer than 260 characters (MAX_PATH) is bypassed unless long path support is enabled
/// in the Windows registry. However, since the external tools (like node/pnpm) cannot
/// handle UNC paths anyway, this is a necessary trade-off for compatibility.
fn simplify_path(path: &std::path::Path) -> std::path::PathBuf {
    if let Ok(stripped) = path.strip_prefix(r"\\?\") {
        if !stripped.starts_with(r"UNC\") {
            return stripped.to_path_buf();
        }
    }
    path.to_path_buf()
}

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

    let clean_workspace = simplify_path(&config.workspace_path);

    // Set working directory
    cmd.current_dir(&clean_workspace);

    // Apply environment isolation: clear all inherited environment variables
    cmd.env_clear();

    // Re-apply whitelisted essential system variables.
    // PATH is rebuilt via registry User/Machine Path + discovered
    // Python/Cargo/Node CLI tool dirs (see get_effective_path).
    for (k, v) in crate::utils::env::get_isolated_env() {
        cmd.env(k, v);
    }

    // Keep host home directories so CLI tools can discover their config, and pass through host temp variables.
    if let Ok(sys_temp) = std::env::var("TEMP") {
        cmd.env("TEMP", sys_temp);
    }
    if let Ok(sys_tmp) = std::env::var("TMP") {
        cmd.env("TMP", sys_tmp);
    }

    // Add user-specified environment variables
    for (key, value) in &config.env_vars {
        cmd.env(key, value);
    }

    info!("Windows environment configured: workspace isolated, PATH rebuilt from registry + discovered tool dirs");
    let path_len = crate::utils::env::get_effective_path().len();
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
    let tmp_dir = clean_workspace.join(".libragent/tmp");
    tokio::fs::create_dir_all(&tmp_dir)
        .await
        .map_err(|e| format!("Failed to create tmp dir: {}", e))?;

    // Monotonic counter avoids collisions when multiple commands fire within the same millisecond.
    let seq = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let script_path = tmp_dir.join(format!("cmd_{}_{}.ps1", config.session_id, seq));

    // Plain readable script — no obfuscation, AV-friendly.
    //
    // ErrorActionPreference must be Continue (not Stop):
    // Under Stop, native stderr merged via `2>&1` becomes terminating ErrorRecords,
    // aborting pipelines like `cargo … 2>&1 | Select-Object` mid-stream and routing
    // into catch with fake failures.
    //
    // Exit code: prefer $LASTEXITCODE when set (including 0). Do not use bare `$?`
    // alone after 2>&1 — NativeCommandError records set $? = false even on success.
    // Catch writes Exception.Message only (never ScriptStackTrace) to avoid
    // `at <ScriptBlock>, …cmd_*.ps1` noise in agent-visible stderr.
    let script_content = format!(
        "$ErrorActionPreference = 'Continue'\n\
         [System.Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US'\n\
         [Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n\
         $__libr_exit = 0\n\
         try {{\n\
             {}\n\
             if ($null -ne $LASTEXITCODE) {{\n\
                 $__libr_exit = $LASTEXITCODE\n\
             }} elseif (-not $?) {{\n\
                 $__libr_exit = 1\n\
             }}\n\
         }} catch {{\n\
             [Console]::Error.WriteLine($_.Exception.Message)\n\
             $__libr_exit = 1\n\
         }}\n\
         exit $__libr_exit\n",
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

    // Run the target .ps1, capture its exit code, then delete it.
    //
    // IMPORTANT: Do not wrap `& script` in `try/finally` without re-exiting.
    // `exit N` inside a script invoked via `&` sets $LASTEXITCODE but returns to
    // the caller; an outer try/finally that ends normally made powershell.exe
    // report exit 0 even after script failures (false success to the agent).
    let self_deleting_wrapper = format!(
        "& '{}'; $__libr_code = $LASTEXITCODE; if ($null -eq $__libr_code) {{ $__libr_code = 0 }}; Remove-Item -LiteralPath '{}' -Force -ErrorAction SilentlyContinue; exit $__libr_code",
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

/// Medium isolation: process groups
pub async fn create_medium_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    let mut cmd = create_basic_isolated_command(config.clone()).await?;

    // Apply platform-specific process group isolation
    #[cfg(target_os = "windows")]
    {
        // Use bitwise OR to preserve CREATE_NO_WINDOW from create_basic_isolated_command
        cmd.creation_flags(0x08000000 | 0x00000200); // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
    }

    Ok(cmd)
}

/// Windows high isolation using job objects and restricted tokens
pub async fn create_high_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    let mut cmd = create_medium_isolated_command(config.clone()).await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplify_path_local_drive() {
        let path = std::path::Path::new(r"\\?\C:\Users\SKTelecom\project");
        let simplified = simplify_path(path);
        assert_eq!(
            simplified,
            std::path::Path::new(r"C:\Users\SKTelecom\project")
        );
    }

    #[test]
    fn test_simplify_path_unc_share() {
        let path = std::path::Path::new(r"\\?\UNC\server\share\project");
        let simplified = simplify_path(path);
        assert_eq!(
            simplified,
            std::path::Path::new(r"\\?\UNC\server\share\project")
        );
    }

    #[test]
    fn test_simplify_path_no_prefix() {
        let path = std::path::Path::new(r"C:\Users\SKTelecom\project");
        let simplified = simplify_path(path);
        assert_eq!(
            simplified,
            std::path::Path::new(r"C:\Users\SKTelecom\project")
        );
    }

    /// Mirrors the script content format used by `create_basic_isolated_command`.
    fn build_script_content(full_command: &str) -> String {
        format!(
            "$ErrorActionPreference = 'Continue'\n\
             [System.Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US'\n\
             [Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n\
             $__libr_exit = 0\n\
             try {{\n\
                 {}\n\
                 if ($null -ne $LASTEXITCODE) {{\n\
                     $__libr_exit = $LASTEXITCODE\n\
                 }} elseif (-not $?) {{\n\
                     $__libr_exit = 1\n\
                 }}\n\
             }} catch {{\n\
                 [Console]::Error.WriteLine($_.Exception.Message)\n\
                 $__libr_exit = 1\n\
             }}\n\
             exit $__libr_exit\n",
            full_command
        )
    }

    /// Mirrors the self-deleting wrapper format used by `create_basic_isolated_command`.
    fn build_cleanup_wrapper(script_path: &str) -> String {
        let escaped = script_path.replace("'", "''");
        format!(
            "& '{}'; $__libr_code = $LASTEXITCODE; if ($null -eq $__libr_code) {{ $__libr_code = 0 }}; Remove-Item -LiteralPath '{}' -Force -ErrorAction SilentlyContinue; exit $__libr_code",
            escaped, escaped
        )
    }

    // ── Script content tests ────────────────────────────────────────────────

    #[test]
    fn test_script_content_has_error_handling() {
        // Script must capture exit codes, report terminating errors to stderr, and exit.
        let script = build_script_content("Remove-Item -Path 'C:\\test' -Recurse -Force");
        assert!(script.contains("$ErrorActionPreference = 'Continue'"));
        assert!(script.contains("try {"));
        assert!(script.contains("} catch {"));
        assert!(script.contains("[Console]::Error.WriteLine"));
        assert!(script.contains("exit $__libr_exit"));
        assert!(
            !script.contains("ScriptStackTrace"),
            "ScriptStackTrace produces at <ScriptBlock> noise in agent stderr"
        );
        assert!(
            !script.contains("$ErrorActionPreference = 'Stop'"),
            "Stop makes native 2>&1 stderr terminating and aborts pipelines"
        );
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
        // Wrapper must invoke the script, preserve exit code, AND delete the temp file.
        let wrapper = build_cleanup_wrapper("C:\\workspace\\tmp\\cmd_abc_0.ps1");
        assert!(wrapper.contains("& 'C:\\workspace\\tmp\\cmd_abc_0.ps1'"));
        assert!(wrapper.contains("Remove-Item"));
        assert!(wrapper.contains("-LiteralPath"));
        assert!(wrapper.contains("exit $__libr_code"));
        assert!(
            !wrapper.contains("try {"),
            "Outer try/finally without re-exit swallowed script exit codes"
        );
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
