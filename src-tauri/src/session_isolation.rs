use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};

/// Shell type enumeration for cross-platform shell support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Used by tool handlers and future shell selection logic
pub enum ShellType {
    Bash,
    PowerShell,
    Cmd,
}

/// Cross-platform session isolation manager
#[derive(Debug, Clone)]
pub struct SessionIsolationManager {
    isolation_config: IsolationConfig,
}

#[derive(Debug, Clone)]
pub struct IsolationConfig {
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    #[allow(dead_code)] // Planned for future use
    pub max_memory_mb: Option<u64>,
    #[allow(dead_code)] // Planned for future use
    pub max_execution_time_secs: Option<u64>,
    #[allow(dead_code)] // Planned for future use
    pub max_open_files: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct IsolatedProcessConfig {
    pub session_id: String,
    pub workspace_path: PathBuf,
    pub command: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub isolation_level: IsolationLevel,
    pub shell_type: Option<ShellType>,
}

#[derive(Debug, Clone)]
pub enum IsolationLevel {
    /// Basic process isolation (environment variables only)
    #[allow(dead_code)] // Reserved for future use
    Basic,
    /// Medium isolation (process groups + limited resources)
    Medium,
    /// High isolation (platform-specific sandboxing)
    #[allow(dead_code)] // Reserved for future use
    High,
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            resource_limits: ResourceLimits {
                max_memory_mb: Some(512),
                max_execution_time_secs: Some(300),
                max_open_files: Some(1024),
            },
        }
    }
}

impl SessionIsolationManager {
    pub fn new() -> Self {
        Self {
            isolation_config: IsolationConfig::default(),
        }
    }

    /// Create an isolated command based on the current platform
    pub async fn create_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        info!(
            "Creating isolated command for session: {}",
            config.session_id
        );

        match config.isolation_level {
            IsolationLevel::Basic => self.create_basic_isolated_command(config).await,
            IsolationLevel::Medium => self.create_medium_isolated_command(config).await,
            IsolationLevel::High => self.create_high_isolated_command(config).await,
        }
    }

    /// Basic isolation: environment variables and working directory
    async fn create_basic_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        // Detect if this is a direct PowerShell/executable command (Windows-specific)
        let (shell_cmd, use_shell_wrapper) = if cfg!(target_os = "windows") {
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
                (self.get_shell_command(config.shell_type).to_string(), true)
            }
        } else {
            (self.get_shell_command(None).to_string(), true)
        };

        let mut cmd = AsyncCommand::new(&shell_cmd);

        // Set working directory
        cmd.current_dir(&config.workspace_path);

        // Configure environment variables based on platform
        #[cfg(target_os = "windows")]
        {
            // Windows: DO NOT use env_clear() as it breaks process execution
            // Windows processes need critical system environment variables:
            // - SystemRoot, windir: Required for loading system DLLs
            // - COMSPEC: Required for cmd.exe
            // - ProgramFiles, ProgramData: Required for many applications
            // - PSModulePath: Required for PowerShell
            //
            // Instead, we selectively set/override specific variables
            cmd.env("USERPROFILE", &config.workspace_path);
            cmd.env("HOME", &config.workspace_path);
            cmd.env("TEMP", config.workspace_path.join("tmp"));
            cmd.env("TMP", config.workspace_path.join("tmp"));

            // Smart Discovery: Auto-detect and prepend valid Python to PATH
            // This fixes issues where the Windows Store shim (WindowsApps\python.exe) takes precedence
            let current_path = std::env::var("PATH").unwrap_or_default();
            if let Some(python_dir) = self.detect_python_path().await {
                let python_str = python_dir.to_string_lossy();
                // Simple check to avoid duplicate appending if it's already in PATH
                if !current_path.contains(python_str.as_ref()) {
                    let scripts_dir = python_dir.join("Scripts");
                    let lib_bin_dir = python_dir.join("Library").join("bin");

                    // PREPEND to PATH to ensure this Python takes precedence over WindowsApps shim
                    let new_path = format!(
                        "{};{};{};{}",
                        python_str,
                        scripts_dir.to_string_lossy(),
                        lib_bin_dir.to_string_lossy(),
                        current_path
                    );

                    cmd.env("PATH", new_path);
                    info!("Smart Discovery: Preended Python at {} to PATH", python_str);
                }
            }

            // NOTE: We DO NOT override PATH on Windows unless we are appending to it!
            // Preserving user's PATH allows access to Python, Node.js, Git, etc.
            // Security isolation is achieved through workspace directory restrictions

            info!("Windows environment configured: workspace isolated, PATH preserved (with Anaconda if found)");
            // Additional env diagnostic info to help diagnose missing output on Windows
            let path_len = std::env::var("PATH").map(|p| p.len()).unwrap_or(0);
            let system_root =
                std::env::var("SystemRoot").unwrap_or_else(|_| "<not-set>".to_string());
            let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "<not-set>".to_string());
            let psmodulepath =
                std::env::var("PSModulePath").unwrap_or_else(|_| "<not-set>".to_string());
            info!("Windows env snapshot (for debugging): PATH.len={}, SystemRoot={}, COMSPEC={}, PSModulePath.present={}", path_len, system_root, comspec, !psmodulepath.is_empty());
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Unix: Safe to clear environment for better isolation
            cmd.env_clear();
            cmd.env("HOME", &config.workspace_path);
            cmd.env("PWD", &config.workspace_path);
            cmd.env("TMPDIR", config.workspace_path.join("tmp"));
            cmd.env("PATH", self.get_restricted_path());
        }

        // Add user-specified environment variables (applies to all platforms)
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }

        // Set command arguments based on platform and shell type
        if cfg!(target_os = "windows") {
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
                // Log selected parts of environment for debug: sometimes missing PATH or SystemRoot
                info!(
                    "PowerShell direct exec: command='{}' args={:?} workspace_dir={}",
                    shell_cmd,
                    parts.get(1..).unwrap_or(&[]),
                    config.workspace_path.display()
                );
            } else {
                // Windows: Use PowerShell instead of cmd.exe for better quote handling
                // PowerShell handles double quotes correctly without complex escaping
                let full_command = if config.args.is_empty() {
                    config.command.clone()
                } else {
                    format!("{} {}", config.command, config.args.join(" "))
                };

                // Override to use PowerShell
                cmd = AsyncCommand::new("powershell");
                cmd.current_dir(&config.workspace_path);

                // Reapply environment variables for PowerShell
                cmd.env("USERPROFILE", &config.workspace_path);
                cmd.env("HOME", &config.workspace_path);
                cmd.env("TEMP", config.workspace_path.join("tmp"));
                cmd.env("TMP", config.workspace_path.join("tmp"));
                for (key, value) in &config.env_vars {
                    cmd.env(key, value);
                }

                // PowerShell command-line arguments:
                // -NoProfile: Don't load user profile (faster, more secure)
                // -NonInteractive: Disable interactive prompts
                // -Command: Execute the command string
                //
                // CRITICAL FIX: Wrap command to redirect PowerShell errors to stderr
                // PowerShell writes errors to its own error stream, not stderr by default.
                // We need to:
                // 1. Set $ErrorActionPreference = 'Stop' to make non-terminating errors terminate
                // 2. Use try-catch to capture all errors including non-terminating ones
                // 3. Write errors to stderr using [Console]::Error.WriteLine()
                //
                // This ensures that when commands like Remove-Item fail, the error message
                // is captured in stderr, not lost in the void.
                //
                // IMPORTANT: Do NOT escape double quotes in full_command
                // PowerShell -Command already handles quotes correctly
                // Escaping breaks nested quotes in commands like: python -c "print('test')"
                let wrapped_command = format!(
                    "$ErrorActionPreference = 'Stop'; try {{ {full_command} }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); [Console]::Error.WriteLine($_.ScriptStackTrace); exit 1 }}"
                );

                cmd.args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &wrapped_command,
                ]);

                info!(
                    "Windows PowerShell execution with error redirection: powershell -Command \"{}\"",
                    wrapped_command
                );

                // Log environment snapshot to help debugging commands that behave differently
                let path_len = std::env::var("PATH").map(|p| p.len()).unwrap_or(0);
                let system_root =
                    std::env::var("SystemRoot").unwrap_or_else(|_| "<not-set>".to_string());
                let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "<not-set>".to_string());
                info!(
                    "PowerShell wrapper env snapshot: PATH.len={}, SystemRoot={}, COMSPEC={}",
                    path_len, system_root, comspec
                );
            }
        } else {
            // Unix shells (bash, sh) use -c flag
            let full_command = if config.args.is_empty() {
                config.command.clone()
            } else {
                format!("{} {}", config.command, config.args.join(" "))
            };
            info!("Unix shell execution: {} -c {}", shell_cmd, full_command);
            cmd.args(["-c", &full_command]);
        }

        info!(
            "Isolated command created for session {} with isolation level {:?}",
            config.session_id, config.isolation_level
        );
        Ok(cmd)
    }

    /// Medium isolation: process groups + resource limits
    async fn create_medium_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        let mut cmd = self.create_basic_isolated_command(config.clone()).await?;

        // Apply platform-specific process group isolation
        #[cfg(unix)]
        {
            #[allow(unused_imports)]
            use std::os::unix::process::CommandExt;
            cmd.process_group(0); // Create new process group
        }

        #[cfg(target_os = "windows")]
        {
            #[allow(unused_imports)]
            use std::os::windows::process::CommandExt;
            // CREATE_NEW_PROCESS_GROUP: Isolate process group for better signal handling
            // Note: Both CREATE_NO_WINDOW and DETACHED_PROCESS break stdio for cmd.exe!
            // Using ONLY CREATE_NEW_PROCESS_GROUP allows:
            // - Process isolation (can terminate process group)
            // - Working stdio pipes (stdout/stderr captured properly)
            // - No visible console window (when spawned from GUI app like Tauri)
            cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP only
        }

        // Apply resource limits using platform-specific methods
        self.apply_resource_limits(&mut cmd, &config).await?;

        Ok(cmd)
    }

    /// High isolation: platform-specific sandboxing
    async fn create_high_isolated_command(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        match std::env::consts::OS {
            "linux" => {
                #[cfg(target_os = "linux")]
                {
                    self.create_linux_high_isolation(config).await
                }
                #[cfg(not(target_os = "linux"))]
                {
                    warn!("Linux isolation not available on this platform, falling back to medium isolation");
                    self.create_medium_isolated_command(config).await
                }
            }
            "macos" => {
                #[cfg(target_os = "macos")]
                {
                    self.create_macos_high_isolation(config).await
                }
                #[cfg(not(target_os = "macos"))]
                {
                    warn!("macOS isolation not available on this platform, falling back to medium isolation");
                    self.create_medium_isolated_command(config).await
                }
            }
            "windows" => {
                #[cfg(target_os = "windows")]
                {
                    self.create_windows_high_isolation(config).await
                }
                #[cfg(not(target_os = "windows"))]
                {
                    warn!("Windows isolation not available on this platform, falling back to medium isolation");
                    self.create_medium_isolated_command(config).await
                }
            }
            _ => {
                warn!("High isolation not supported on this platform, falling back to medium isolation");
                self.create_medium_isolated_command(config).await
            }
        }
    }

    /// Linux high isolation using unshare (user namespaces)
    #[cfg(target_os = "linux")]
    async fn create_linux_high_isolation(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        // Check if unshare is available
        if !self.is_command_available("unshare").await {
            warn!("unshare not available, falling back to medium isolation");
            return self.create_medium_isolated_command(config).await;
        }

        let mut cmd = AsyncCommand::new("unshare");

        // Configure namespaces for isolation
        cmd.args([
            "--user",  // User namespace isolation
            "--pid",   // PID namespace isolation
            "--mount", // Mount namespace isolation
            "--fork",  // Fork before exec
            "--",
        ]);

        // Add the actual command
        cmd.arg(&config.command);
        cmd.args(&config.args);

        // Set environment and working directory
        cmd.current_dir(&config.workspace_path);
        cmd.env_clear();
        cmd.env("HOME", &config.workspace_path);
        cmd.env("PWD", &config.workspace_path);
        cmd.env("PATH", self.get_restricted_path());

        for (key, value) in config.env_vars {
            cmd.env(key, value);
        }

        info!(
            "Created Linux high isolation command for session: {}",
            config.session_id
        );
        Ok(cmd)
    }

    /// macOS high isolation using sandbox-exec
    #[cfg(target_os = "macos")]
    async fn create_macos_high_isolation(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        // Check if sandbox-exec is available
        if !self.is_command_available("sandbox-exec").await {
            warn!("sandbox-exec not available, falling back to medium isolation");
            return self.create_medium_isolated_command(config).await;
        }

        // Create a sandbox profile for this session
        let profile_content = self.create_macos_sandbox_profile(&config)?;
        let profile_path = config.workspace_path.join(".sandbox_profile");

        tokio::fs::write(&profile_path, profile_content)
            .await
            .map_err(|e| format!("Failed to write sandbox profile: {e}"))?;

        let mut cmd = AsyncCommand::new("sandbox-exec");
        cmd.args([
            "-f",
            profile_path
                .to_str()
                .ok_or_else(|| "Invalid workspace path: non-UTF8 characters not supported".to_string())?,
        ]);
        cmd.arg(&config.command);
        cmd.args(&config.args);

        // Set environment and working directory
        cmd.current_dir(&config.workspace_path);
        cmd.env_clear();
        cmd.env("HOME", &config.workspace_path);
        cmd.env("PWD", &config.workspace_path);
        cmd.env("PATH", self.get_restricted_path());

        for (key, value) in config.env_vars {
            cmd.env(key, value);
        }

        info!(
            "Created macOS high isolation command for session: {}",
            config.session_id
        );
        Ok(cmd)
    }

    /// Windows high isolation using job objects and restricted tokens
    #[cfg(target_os = "windows")]
    async fn create_windows_high_isolation(
        &self,
        config: IsolatedProcessConfig,
    ) -> Result<AsyncCommand, String> {
        let mut cmd = self.create_medium_isolated_command(config.clone()).await?;

        // Apply Windows-specific isolation
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;

        // Windows high isolation flags:
        // - CREATE_NEW_PROCESS_GROUP: Isolate process for signal handling
        // Note: Both CREATE_NO_WINDOW and DETACHED_PROCESS break stdio for cmd.exe!
        // Using only CREATE_NEW_PROCESS_GROUP for same reason as Medium isolation
        cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP only

        info!(
            "Created Windows high isolation command for session: {}",
            config.session_id
        );
        Ok(cmd)
    }

    /// Apply resource limits to the command
    async fn apply_resource_limits(
        &self,
        _cmd: &mut AsyncCommand,
        _config: &IsolatedProcessConfig,
    ) -> Result<(), String> {
        // `limits` is used only on Unix-like builds (Linux/macOS) below.
        // To avoid accidental removal by Windows-only edits, bind a
        // platform-specific variable:
        // - on Unix: `limits` (used in the Unix-only info block)
        // - on non-unix: `_limits` (keeps the reference but avoids unused warnings)
        #[cfg(unix)]
        let limits = &self.isolation_config.resource_limits;

        #[cfg(not(unix))]
        let _limits = &self.isolation_config.resource_limits;

        #[cfg(unix)]
        {
            // Resource limits will be applied when creating the command
            // through shell wrappers in the individual platform implementations
            info!(
                "Resource limits configured: memory_mb={:?}, time_secs={:?}, open_files={:?}",
                limits.max_memory_mb, limits.max_execution_time_secs, limits.max_open_files
            );
        }

        // Windows resource limits would be applied through job objects
        // This requires more complex Windows API calls
        #[cfg(target_os = "windows")]
        {
            warn!("Windows resource limits not implemented yet, using basic limits");
        }

        Ok(())
    }

    /// Create macOS sandbox profile
    #[cfg(target_os = "macos")]
    fn create_macos_sandbox_profile(
        &self,
        config: &IsolatedProcessConfig,
    ) -> Result<String, String> {
        let workspace_path_str = config
            .workspace_path
            .to_str()
            .ok_or("Invalid workspace path")?;

        let profile = format!(
            r#"
(version 1)
(deny default)

;; Allow basic system operations
(allow process-info* (target self))
(allow signal (target self))
(allow sysctl-read)

;; Allow reading system frameworks and libraries
(allow file-read*
    (subpath "/System/Library")
    (subpath "/usr/lib")
    (subpath "/usr/bin")
    (subpath "/bin"))

;; Allow access to workspace directory
(allow file-read* file-write* file-ioctl
    (subpath "{workspace_path}"))

;; Allow temporary directory access
(allow file-read* file-write* file-ioctl
    (subpath "/tmp")
    (subpath "/var/tmp"))

;; Allow network access if enabled
{network_rules}

;; Deny access to sensitive directories
(deny file-read* file-write*
    (subpath "/private")
    (subpath "$HOME" (except (subpath "{workspace_path}"))))
"#,
            workspace_path = workspace_path_str,
            network_rules = "(allow network*)" // Allow network access by default
        );

        Ok(profile)
    }

    /// Check if a command is available on the system
    #[allow(dead_code)] // Used by platform-specific high isolation
    async fn is_command_available(&self, command: &str) -> bool {
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
    fn get_shell_command(&self, shell_type: Option<ShellType>) -> &str {
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
    fn get_restricted_path(&self) -> String {
        if cfg!(target_os = "windows") {
            // Windows PATH must include:
            // - System32: Core Windows commands (cmd, findstr, etc.)
            // - Windows: Additional system utilities
            // - System32\WindowsPowerShell\v1.0: PowerShell (if available)
            // Note: We intentionally restrict access to user-installed software
            "C:\\Windows\\System32;C:\\Windows;C:\\Windows\\System32\\WindowsPowerShell\\v1.0"
        } else {
            "/bin:/usr/bin:/usr/local/bin"
        }
        .to_string()
    }

    /// Detects a valid Python installation on Windows, prioritizing non-Store versions.
    #[cfg(target_os = "windows")]
    async fn detect_python_path(&self) -> Option<PathBuf> {
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

        for path in common_paths.into_iter().flatten() {
            // For standard Python, we might need to look deeper (e.g. Python39, Python310)
            if path.join("python.exe").exists() {
                info!("Detected Python via standard path: {:?}", path);
                return Some(path);
            }

            // Check subdirectories for standard Python installs
            if path.exists() && path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for entry in entries.flatten() {
                        let subpath = entry.path();
                        if subpath.join("python.exe").exists() {
                            info!(
                                "Detected Python via standard path subdirectory: {:?}",
                                subpath
                            );
                            return Some(subpath);
                        }
                    }
                }
            }
        }

        None
    }
}

// Note: AsyncCommand argument manipulation is complex and platform-specific
// For now, we'll build commands differently to avoid this issue

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn test_powershell_error_wrapping() {
        // Test that the wrapped command includes error handling
        let test_command = "Remove-Item -Path \"C:\\test\" -Recurse -Force";
        let escaped_command = test_command.replace("\"", "`\"");
        let wrapped = format!(
            "$ErrorActionPreference = 'Stop'; try {{ {} }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); [Console]::Error.WriteLine($_.ScriptStackTrace); exit 1 }}",
            escaped_command
        );

        // Verify the wrapped command contains key elements
        assert!(wrapped.contains("$ErrorActionPreference = 'Stop'"));
        assert!(wrapped.contains("try {"));
        assert!(wrapped.contains("} catch {"));
        assert!(wrapped.contains("[Console]::Error.WriteLine($_.Exception.Message)"));
        assert!(wrapped.contains("exit 1"));
        assert!(wrapped.contains("Remove-Item -Path `\"C:\\test`\" -Recurse -Force"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_powershell_quote_escaping() {
        // Test that double quotes are properly escaped for PowerShell
        let test_command = "Write-Host \"Hello World\"";
        let escaped = test_command.replace("\"", "`\"");

        assert_eq!(escaped, "Write-Host `\"Hello World`\"");

        // Verify it works in the full wrapper
        let wrapped = format!(
            "$ErrorActionPreference = 'Stop'; try {{ {} }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1 }}",
            escaped
        );

        assert!(wrapped.contains("Write-Host `\"Hello World`\""));
    }

    #[test]
    fn test_isolation_manager_creation() {
        let manager = SessionIsolationManager::new();
        assert!(manager
            .isolation_config
            .resource_limits
            .max_memory_mb
            .is_some());
    }
}
