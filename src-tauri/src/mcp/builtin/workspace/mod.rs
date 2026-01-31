use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::info;

use super::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::services::SecureFileManager;
use crate::session::SessionManager;

/// Shell type enumeration for cross-platform shell support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    PowerShell,
    Cmd,
}

impl ShellType {
    /// Get shell command for spawning
    pub fn command(&self) -> &str {
        match self {
            ShellType::Bash => "bash",
            ShellType::PowerShell => "powershell.exe",
            ShellType::Cmd => "cmd.exe",
        }
    }

    /// Check if this is a Windows shell
    pub fn is_windows(&self) -> bool {
        matches!(self, ShellType::PowerShell | ShellType::Cmd)
    }
}

// Platform-specific persistent shell tool name
#[cfg(unix)]
pub const PERSISTENT_SHELL_TOOL: &str = "runInPersistentShell";
#[cfg(windows)]
pub const PERSISTENT_SHELL_TOOL: &str = "runInPersistentPowerShell";

// Module imports
pub mod code_execution;
pub mod export_operations;
pub mod file_operations;
pub mod handlers; // NEW: Organized handler modules
pub mod persistent_shell;
pub mod persistent_shell_manager;
pub mod terminal_manager;
pub mod tools;
pub mod ui_resources;
pub mod utils;

#[cfg(test)]
mod test_output_visibility;

/// Pending execution state (server-side only)
/// Stores metadata for shell commands awaiting user input
#[derive(Debug, Clone)]
pub struct PendingShellExecution {
    pub execution_id: String,
    pub session_id: String,
    pub executable_command: String, // Command to execute (may include -S flag)
    pub display_command: String,    // Sanitized version for logs/UI
    pub run_mode: String,           // "sync" or "async" from 1st call
    pub timeout: u64,               // Command execution timeout in seconds
    pub encryption_nonce: String,   // Nonce for client-side input obfuscation
    pub created_at: DateTime<Utc>,
}

/// Thread-safe storage for pending shell executions
/// Manages a map of execution_id -> PendingShellExecution
#[derive(Debug)]
pub struct PendingExecutions(Mutex<HashMap<String, PendingShellExecution>>);

impl Default for PendingExecutions {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingExecutions {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    pub fn insert(&self, exec: PendingShellExecution) {
        self.0
            .lock()
            .unwrap()
            .insert(exec.execution_id.clone(), exec);
    }

    pub fn remove(&self, id: &str) -> Option<PendingShellExecution> {
        self.0.lock().unwrap().remove(id)
    }
}

#[derive(Debug)]
pub struct WorkspaceServer {
    pub(crate) session_id: String,
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) isolation_manager: crate::session_isolation::SessionIsolationManager,
    pub(crate) process_registry: terminal_manager::ProcessRegistry,
    pub(crate) pending_executions: Arc<PendingExecutions>,
    pub(crate) shell_manager: Arc<persistent_shell_manager::PersistentShellManager>,
    pub(crate) context_cache: Arc<tokio::sync::RwLock<Option<(String, std::time::Instant)>>>,
}

impl WorkspaceServer {
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        info!("WorkspaceServer created for session: {}", session_id);
        let process_registry = terminal_manager::create_process_registry();

        // Start cleanup task for old processes
        Self::start_cleanup_task(process_registry.clone());

        let isolation_manager = crate::session_isolation::SessionIsolationManager::new();
        // Create PersistentShellManager with access to isolation logic
        let shell_manager = Arc::new(persistent_shell_manager::PersistentShellManager::new(
            Arc::new(isolation_manager.clone()),
        ));

        Self {
            session_id,
            session_manager,
            isolation_manager,
            process_registry,
            pending_executions: Arc::new(PendingExecutions::new()),
            shell_manager,
            context_cache: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Invalidate the service context cache (call after state changes)
    pub(crate) async fn invalidate_context_cache(&self) {
        match self.context_cache.try_write() {
            Ok(mut guard) => {
                *guard = None;
                tracing::debug!("Workspace service context cache invalidated");
            }
            Err(_) => {
                tracing::warn!("Failed to invalidate context cache - lock held by another task");
            }
        }
    }

    /// Start background task to cleanup old processes (24-hour retention)
    fn start_cleanup_task(registry: terminal_manager::ProcessRegistry) {
        tokio::spawn(async move {
            use std::time::Duration;
            let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Every hour

            loop {
                interval.tick().await;
                Self::cleanup_old_processes(&registry).await;
            }
        });
    }

    /// Clean up processes older than 24 hours
    async fn cleanup_old_processes(registry: &terminal_manager::ProcessRegistry) {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
        let mut reg = registry.write().await;

        let to_remove: Vec<String> = reg
            .entries
            .values()
            .filter(|e| {
                matches!(
                    e.status,
                    terminal_manager::ProcessStatus::Finished
                        | terminal_manager::ProcessStatus::Failed
                        | terminal_manager::ProcessStatus::Killed
                )
            })
            .filter(|e| e.finished_at.is_some_and(|t| t < cutoff))
            .map(|e| e.id.clone())
            .collect();

        for id in to_remove {
            if let Some(entry) = reg.entries.remove(&id) {
                // Remove cancellation token
                reg.cancellation_tokens.remove(&id);
                // Remove output directory
                if let Some(parent) = std::path::PathBuf::from(&entry.stdout_path).parent() {
                    let _ = tokio::fs::remove_dir_all(parent).await;
                }
                // Log poll statistics for monitoring
                tracing::info!(
                    "Cleaned up old process: {} (polls: {}, consecutive_running_polls: {})",
                    id,
                    entry.poll_count,
                    entry.consecutive_running_polls
                );
            }
        }
    }

    /// Session cleanup: terminate and clean up all processes for a session
    #[allow(dead_code)] // Will be called by session manager
    pub async fn on_session_end(&self, session_id: &str) {
        info!("Cleaning up processes for session: {}", session_id);
        let mut reg = self.process_registry.write().await;

        // Get all processes for this session
        let session_processes: Vec<String> = reg
            .entries
            .values()
            .filter(|e| e.session_id == session_id)
            .map(|e| e.id.clone())
            .collect();

        let process_count = session_processes.len();

        for id in session_processes {
            // Cancel process via token first
            if let Some(token) = reg.cancellation_tokens.get(&id) {
                token.cancel();
            }

            if let Some(entry) = reg.entries.remove(&id) {
                // Remove cancellation token
                reg.cancellation_tokens.remove(&id);

                // Kill running processes
                if let Some(pid) = entry.pid {
                    if matches!(entry.status, terminal_manager::ProcessStatus::Running) {
                        info!("Killing running process {} (PID {})", id, pid);

                        #[cfg(unix)]
                        {
                            // Unix: send SIGTERM
                            use std::process::Command;
                            let _ = Command::new("kill")
                                .arg("-TERM")
                                .arg(pid.to_string())
                                .output();
                        }

                        #[cfg(windows)]
                        {
                            // Windows: use taskkill
                            use std::process::Command;
                            let _ = Command::new("taskkill")
                                .args(["/PID", &pid.to_string(), "/F"])
                                .output();
                        }
                    }
                }

                // Remove output directory
                let output_dir = std::path::PathBuf::from(&entry.stdout_path)
                    .parent()
                    .map(|p| p.to_path_buf());
                if let Some(dir) = output_dir {
                    let _ = tokio::fs::remove_dir_all(&dir).await;
                    info!("Removed output directory for process: {}", id);
                }
            }
        }

        info!(
            "Cleaned up {} processes for session {}",
            process_count, session_id
        );

        // Cleanup persistent shell for this session
        if let Err(e) = self.shell_manager.terminate_shell(session_id).await {
            tracing::warn!(
                "Failed to terminate persistent shell for session {}: {}",
                session_id,
                e
            );
        }
    }

    // Common utility methods
    pub fn get_workspace_dir(&self, session_id: &str) -> std::path::PathBuf {
        // Resolve path dynamically via SessionManager to support per-session overrides
        self.session_manager
            .get_session_workspace_dir_by_id(session_id)
    }

    pub fn get_file_manager(&self, session_id: Option<String>) -> Arc<SecureFileManager> {
        // Use provided session_id or fallback to server's session_id
        // NOTE: The server's session_id is likely "default" due to singleton initialization
        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());

        let workspace_dir = self.get_workspace_dir(&target_session_id);
        Arc::new(SecureFileManager::new_with_base_dir(workspace_dir))
    }

    /// Validate path with security checks (helper for file operations)
    pub fn validate_path_with_error(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let file_manager = self.get_file_manager(session_id);
        file_operations::utils::validate_path_with_error(&file_manager, path_str)
    }

    #[allow(dead_code)]
    fn get_workspace_tree(&self, path: &str, max_depth: usize) -> String {
        use std::fs;

        fn build_tree(
            dir: &std::path::Path,
            prefix: &str,
            depth: usize,
            max_depth: usize,
        ) -> String {
            if depth >= max_depth {
                return String::new();
            }

            let mut result = String::new();
            if let Ok(entries) = fs::read_dir(dir) {
                let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                entries.sort_by_key(|e| e.file_name());

                let mut limited_entries = entries.iter().take(10).peekable();

                while let Some(entry) = limited_entries.next() {
                    let is_last = limited_entries.peek().is_none();
                    let connector = if is_last { "└── " } else { "├── " };
                    let name = entry.file_name().to_string_lossy().to_string();

                    result.push_str(&format!("{prefix}{connector}{name}\n"));

                    if entry.path().is_dir() {
                        let new_prefix =
                            format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                        if depth < max_depth - 1 {
                            result.push_str(&build_tree(
                                &entry.path(),
                                &new_prefix,
                                depth + 1,
                                max_depth,
                            ));
                        }
                    }
                }
            }
            result
        }

        build_tree(std::path::Path::new(path), "", 0, max_depth)
    }
    pub fn tools_static() -> Vec<MCPTool> {
        let mut tools = Vec::new();
        tools.extend(tools::file_tools());
        tools.extend(tools::code_tools());
        tools.extend(tools::export_tools());
        tools.extend(tools::terminal_tools());
        tools
    }

    /// Get metadata statically
    pub fn metadata_static() -> crate::mcp::types::BuiltinServerMetadata {
        crate::mcp::types::BuiltinServerMetadata {
            display_name: "Workspace".to_string(),
            description: "Execute shell commands and manage background processes".to_string(),
            icon: None,
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    fn name(&self) -> &str {
        "workspace"
    }

    fn description(&self) -> &str {
        "Integrated workspace for file operations and code execution"
    }

    fn display_name(&self) -> String {
        "Workspace".to_string()
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        // Get session-specific workspace directory
        let session_id = if let Some(opts) = options {
            opts.get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or(&self.session_id)
                .to_string()
        } else {
            self.session_id.clone()
        };

        // Check cache first
        const CACHE_TTL_SECS: u64 = 5;
        if let Ok(guard) = self.context_cache.try_read() {
            if let Some((cached_prompt, last_update)) = guard.as_ref() {
                if last_update.elapsed().as_secs() < CACHE_TTL_SECS {
                    return ServiceContext {
                        context_prompt: cached_prompt.clone(),
                        structured_state: Some(json!({
                            "cached": true,
                            "session_id": session_id
                        })),
                    };
                }
            }
        }

        let workspace_dir_path = self.get_workspace_dir(&session_id);
        let workspace_dir = workspace_dir_path.to_string_lossy().to_string();

        // Platform information
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let shell = detect_shell(os);

        // Get current shell CWD
        let shell_cwd = if let Some(cwd) = self.shell_manager.get_shell_cwd(&session_id).await {
            // Convert to relative path if within workspace for better readability
            if cwd.starts_with(&workspace_dir) {
                cwd.replacen(&workspace_dir, ".", 1)
            } else {
                cwd
            }
        } else {
            ".".to_string()
        };

        // ✅ ENHANCED: Get running processes with IDs and commands for AI visibility
        let (running_count, total_count, running_processes_text) = {
            match self.process_registry.try_read() {
                Ok(reg) => {
                    let processes: Vec<(String, String)> = reg
                        .entries
                        .values()
                        .filter(|e| e.session_id == session_id)
                        .filter(|e| matches!(e.status, terminal_manager::ProcessStatus::Running))
                        .take(5) // Limit to prevent context bloat
                        .map(|e| (e.id.clone(), e.command.clone()))
                        .collect();

                    let running_count = processes.len();
                    let total_count = reg
                        .entries
                        .values()
                        .filter(|e| e.session_id == session_id)
                        .count();

                    let running_text = if running_count == 0 {
                        "None".to_string()
                    } else {
                        let process_list = processes
                            .iter()
                            .map(|(id, cmd)| {
                                // Truncate command if too long (safe string slicing)
                                let display_cmd = crate::utils::truncate_chars(cmd, 77);
                                format!("  • {} - {}", id, display_cmd)
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        format!("\n{}", process_list)
                    };

                    (running_count, total_count, running_text)
                }
                Err(_) => {
                    // Lock is held by another task, return defaults to avoid blocking
                    tracing::debug!("Could not acquire process registry lock for service context");
                    (0, 0, "None".to_string())
                }
            }
        };

        info!(
            "Workspace service context - workspace_dir: {}, shell_cwd: {}, running processes: {}, total: {}, platform: {}/{}/{}",
            workspace_dir,
            shell_cwd,
            running_count,
            total_count,
            os,
            arch,
            shell
        );

        let context_prompt = format!(
            "## Workspace

**Workspace Root**: {}
**Persistent Shell CWD**: {}
**Platform**: {} / {} using {}

**Background Processes**:
- Running: {}{}
- Total: {}

💡 Use pollProcess(processId) to check status or listProcesses() to see all (including full commands).",
            workspace_dir, shell_cwd, os, arch, shell, running_count, running_processes_text, total_count
        );

        // Update cache
        if let Ok(mut guard) = self.context_cache.try_write() {
            *guard = Some((context_prompt.clone(), std::time::Instant::now()));
        }

        ServiceContext {
            context_prompt,
            structured_state: Some(json!({
                "workspace_dir": workspace_dir,
                "shell_cwd": shell_cwd,
                "platform": {
                    "os": os,
                    "arch": arch,
                    "shell": shell
                },
                "processes": {
                    "running": running_count,
                    "total": total_count,
                },
                "shell_active": !shell_cwd.is_empty(),
                "tools_count": self.tools().len()
            })),
        }
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        info!("Workspace tool called: {} with args: {:?}", tool_name, args);

        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());

        match tool_name {
            // File operation tools
            "readFile" => self.handle_read_file(args, session_id).await,
            "writeFile" => self.handle_write_file(args, session_id).await,
            "deleteFile" => self.handle_delete_file(args, session_id).await,
            "listDirectory" => self.handle_list_directory(args, session_id).await,
            "editFile" => self.handle_edit_file(args, session_id).await,
            "editFileMulti" => self.handle_edit_file_multi(args, session_id).await,
            "previewReplacement" => self.handle_preview_replacement(args, session_id).await,
            "importFile" => self.handle_import_file(args, session_id).await,
            "searchLineInFile" => self.handle_search_line_in_file(args, session_id).await,
            "searchFiles" => self.handle_search_files(args, session_id).await,
            "editLineInFile" => self.handle_edit_line_in_file(args, session_id).await,
            // Code execution tools
            // Note: Python/TypeScript execution were removed from the public tool
            // interface to avoid external runtime dependencies and to prevent
            // agents from controlling isolation/permissions. Only shell
            // execution remains exposed below.
            // PRIMARY isolated shell execution tools (recommended)
            #[cfg(unix)]
            "runShell" => self.handle_run_shell(args, &target_session_id).await,
            #[cfg(windows)]
            "runPowerShell" => self.handle_run_shell(args, &target_session_id).await,
            // ADVANCED persistent shell execution tools (for state preservation)
            #[cfg(unix)]
            "runInPersistentShell" => self.handle_execute_shell(args, &target_session_id).await,
            #[cfg(windows)]
            "runInPersistentPowerShell" => {
                self.handle_execute_shell(args, &target_session_id).await
            }
            // CMD execution tools (Windows only, alternative to PowerShell)
            #[cfg(windows)]
            "runCmd" => self.handle_run_shell(args, &target_session_id).await,
            #[cfg(windows)]
            "runInPersistentCmd" => self.handle_execute_shell(args, &target_session_id).await,
            // Background process execution (platform-agnostic)
            "spawnProcess" => self.handle_spawn_process(args, &target_session_id).await,
            // Interactive shell execution (2nd tool for user input)
            "executePendingShell" => {
                self.handle_execute_pending_shell(args, &target_session_id)
                    .await
            }
            // Cancel pending execution (UI callback tool)
            "cancelPendingExecution" => {
                self.handle_cancel_pending_execution(args, &target_session_id)
                    .await
            }
            // Export tools
            "exportFile" => self.handle_export_file(args, session_id).await,
            "exportZip" => self.handle_export_zip(args, session_id).await,
            // Terminal/Process management tools
            "pollProcess" => self.handle_poll_process(args, &target_session_id).await,
            "readProcessOutput" => {
                self.handle_read_process_output(args, &target_session_id)
                    .await
            }
            "listProcesses" => self.handle_list_processes(args, &target_session_id).await,
            "stopProcess" => self.handle_stop_process(args, &target_session_id).await,

            _ => Err(format!("Tool '{tool_name}' not found")),
        }
    }
}

/// Detect default shell for the platform
fn detect_shell(os: &str) -> String {
    match os {
        "windows" => {
            // Check for PowerShell vs CMD
            if std::env::var("PSModulePath").is_ok() {
                "powershell".to_string()
            } else {
                "cmd".to_string()
            }
        }
        "macos" | "linux" => {
            // Check SHELL environment variable
            std::env::var("SHELL")
                .ok()
                .and_then(|shell_path| shell_path.split('/').next_back().map(|s| s.to_string()))
                .unwrap_or_else(|| "bash".to_string())
        }
        _ => "unknown".to_string(),
    }
}
