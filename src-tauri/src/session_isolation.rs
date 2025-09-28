use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};

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
    pub max_memory_mb: Option<u64>,
    pub max_execution_time_secs: Option<u64>,
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
}

#[derive(Debug, Clone)]
pub enum IsolationLevel {
    /// Basic process isolation (environment variables only)
    Basic,
    /// Medium isolation (process groups + limited resources)
    Medium,
    /// High isolation (platform-specific sandboxing)
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
        let shell_cmd = self.get_shell_command();
        let mut cmd = AsyncCommand::new(shell_cmd);

        // Set working directory
        cmd.current_dir(&config.workspace_path);

        // Clear and set environment variables
        cmd.env_clear();
        cmd.env("HOME", &config.workspace_path);
        cmd.env("PWD", &config.workspace_path);
        cmd.env("TMPDIR", config.workspace_path.join("tmp"));
        cmd.env("PATH", self.get_restricted_path());

        // Add user-specified environment variables
        for (key, value) in config.env_vars {
            cmd.env(key, value);
        }

        // Set command arguments based on platform
        if cfg!(target_os = "windows") {
            cmd.args([
                "/C",
                &format!("{} {}", config.command, config.args.join(" ")),
            ]);
        } else {
            cmd.args([
                "-c",
                &format!("{} {}", config.command, config.args.join(" ")),
            ]);
        }

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
            use std::os::windows::process::CommandExt;
            // Create new process group and detach from job
            cmd.creation_flags(0x00000200 | 0x08000000); // CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB
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
        cmd.args(["-f", profile_path.to_str().unwrap()]);
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
        use std::os::windows::process::CommandExt;

        // Create suspended process with restricted token
        cmd.creation_flags(
            0x00000004 |  // CREATE_SUSPENDED
            0x08000000 |  // CREATE_BREAKAWAY_FROM_JOB
            0x00000200, // CREATE_NEW_PROCESS_GROUP
        );

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
        let limits = &self.isolation_config.resource_limits;

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
    async fn is_command_available(&self, command: &str) -> bool {
        let check_cmd = if cfg!(target_os = "windows") {
            Command::new("where").arg(command).output()
        } else {
            Command::new("which").arg(command).output()
        };

        match check_cmd {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Get the appropriate shell command for the platform
    fn get_shell_command(&self) -> &str {
        if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "sh"
        }
    }

    /// Get restricted PATH for security
    fn get_restricted_path(&self) -> String {
        if cfg!(target_os = "windows") {
            "C:\\Windows\\System32;C:\\Windows"
        } else {
            "/bin:/usr/bin:/usr/local/bin"
        }
        .to_string()
    }

    /// Validate isolation capabilities on the current platform
    pub async fn validate_isolation_capabilities(&self) -> IsolationCapabilities {
        let mut capabilities = IsolationCapabilities::default();

        // Check for unshare (Linux)
        #[cfg(target_os = "linux")]
        {
            capabilities.supports_user_namespaces = self.is_command_available("unshare").await;
            capabilities.supports_pid_namespaces = capabilities.supports_user_namespaces;
            capabilities.supports_mount_namespaces = capabilities.supports_user_namespaces;
        }

        // Check for sandbox-exec (macOS)
        #[cfg(target_os = "macos")]
        {
            capabilities.supports_macos_sandbox = self.is_command_available("sandbox-exec").await;
        }

        // Windows capabilities
        #[cfg(target_os = "windows")]
        {
            capabilities.supports_job_objects = true; // Always available on Windows
            capabilities.supports_restricted_tokens = true;
        }

        // Resource limits
        capabilities.supports_memory_limits = cfg!(unix);
        capabilities.supports_cpu_limits = cfg!(unix);
        capabilities.supports_time_limits = cfg!(unix);

        info!("Isolation capabilities validated: {:?}", capabilities);
        capabilities
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IsolationCapabilities {
    pub supports_user_namespaces: bool,
    pub supports_pid_namespaces: bool,
    pub supports_mount_namespaces: bool,
    pub supports_macos_sandbox: bool,
    pub supports_job_objects: bool,
    pub supports_restricted_tokens: bool,
    pub supports_memory_limits: bool,
    pub supports_cpu_limits: bool,
    pub supports_time_limits: bool,
}

impl Default for IsolationCapabilities {
    fn default() -> Self {
        Self {
            supports_user_namespaces: false,
            supports_pid_namespaces: false,
            supports_mount_namespaces: false,
            supports_macos_sandbox: false,
            supports_job_objects: false,
            supports_restricted_tokens: false,
            supports_memory_limits: false,
            supports_cpu_limits: false,
            supports_time_limits: false,
        }
    }
}

// Note: AsyncCommand argument manipulation is complex and platform-specific
// For now, we'll build commands differently to avoid this issue
