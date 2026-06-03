use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::info;

use super::BuiltinMCPServer;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::{ContextVolatility, MCPResult, ServiceContext};
use crate::mcp::MCPTool;
use crate::repositories::SessionRepository;
use crate::services::SecureFileManager;
use crate::session::SessionManager;

// Platform-specific persistent shell tool name
#[cfg(unix)]
pub const PERSISTENT_SHELL_TOOL: &str = "runInPersistentShell";
#[cfg(windows)]
pub const PERSISTENT_SHELL_TOOL: &str = "runInPersistentPowerShell";

// Platform-specific one-shot shell tool name
#[cfg(unix)]
pub const RUN_SHELL_TOOL: &str = "runShell";
#[cfg(windows)]
pub const RUN_SHELL_TOOL: &str = "runPowerShell";
pub(crate) const SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL: &str = "submitInteractiveShellInput";
pub(crate) const CANCEL_INTERACTIVE_SHELL_INPUT_INTERNAL: &str = "cancelInteractiveShellInput";
pub(crate) const INTERACTIVE_SHELL_INPUT_TIMEOUT_SECS: u64 = 300;
pub(crate) const INTERACTIVE_SHELL_INPUT_MAX_BYTES: usize = 65_536;

// Module imports
pub mod code_execution;
pub mod export_operations;
pub mod file_operations;
pub mod handlers; // NEW: Organized handler modules
pub mod persistent_shell;
pub mod terminal_manager;
pub mod tools;
pub mod ui_resources;
pub mod utils;

#[cfg(test)]
mod test_output_visibility;

/// Pending execution state (server-side only)
/// Stores metadata for shell commands awaiting user input
pub enum PendingShellInputResolution {
    Submitted(String),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingExecutionLookupError {
    SessionMismatch,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InteractiveShellInputType {
    Text,
    Password,
}

impl InteractiveShellInputType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Password => "password",
        }
    }
}

impl std::fmt::Display for InteractiveShellInputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub struct PendingShellExecution {
    pub execution_id: String,
    pub session_id: String,
    pub executable_command: String, // Command to execute (may include -S flag)
    pub display_command: String,    // Sanitized version for logs/UI
    pub run_mode: String,           // "sync" or "async" from 1st call
    pub timeout: u64,               // Command execution timeout in seconds
    pub created_at: DateTime<Utc>,
    pub prompt: String,
    pub input_type: InteractiveShellInputType,
    pub response_tx: Option<oneshot::Sender<PendingShellInputResolution>>,
}

impl std::fmt::Debug for PendingShellExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingShellExecution")
            .field("execution_id", &self.execution_id)
            .field("session_id", &self.session_id)
            .field("executable_command", &self.executable_command)
            .field("display_command", &self.display_command)
            .field("run_mode", &self.run_mode)
            .field("timeout", &self.timeout)
            .field("created_at", &self.created_at)
            .field("prompt", &self.prompt)
            .field("input_type", &self.input_type)
            .finish_non_exhaustive()
    }
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

    pub fn remove_if_session_matches(
        &self,
        id: &str,
        session_id: &str,
    ) -> Result<Option<PendingShellExecution>, PendingExecutionLookupError> {
        let mut map = self.0.lock().unwrap();
        match map.get(id) {
            None => Ok(None),
            Some(pending) if pending.session_id != session_id => {
                Err(PendingExecutionLookupError::SessionMismatch)
            }
            Some(_) => Ok(map.remove(id)),
        }
    }

    pub fn remove_for_session(&self, session_id: &str) -> Vec<PendingShellExecution> {
        let mut map = self.0.lock().unwrap();
        let ids = map
            .iter()
            .filter(|(_, pending)| pending.session_id == session_id)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();

        ids.into_iter()
            .filter_map(|id| map.remove(&id))
            .collect::<Vec<_>>()
    }

    /// Get count of pending executions (for monitoring)
    pub fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    /// Cleanup expired pending executions
    pub fn cleanup_expired(&self, ttl_seconds: u64) {
        let mut map = self.0.lock().unwrap();
        let now = chrono::Utc::now();
        let ttl_limit = i64::try_from(ttl_seconds).unwrap_or(i64::MAX);
        map.retain(|_, exec| {
            let age = now.signed_duration_since(exec.created_at);
            age.num_seconds() < ttl_limit
        });
    }
}

#[derive(Debug)]
pub struct WorkspaceServer {
    pub(crate) session_id: String,
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) isolation_manager: crate::session_isolation::SessionIsolationManager,
    pub(crate) process_registry: terminal_manager::ProcessRegistry,
    pub(crate) pending_executions: Arc<PendingExecutions>,
    pub(crate) shell_manager: Arc<persistent_shell::PersistentShellManager>,
    pub(crate) context_cache: Arc<tokio::sync::RwLock<Option<(String, std::time::Instant)>>>,
    cleanup_shutdown: Arc<AtomicBool>,
    cleanup_tasks: Vec<JoinHandle<()>>,
}

impl WorkspaceServer {
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        info!("WorkspaceServer created for session: {}", session_id);
        let process_registry = terminal_manager::create_process_registry();
        let pending_executions = Arc::new(PendingExecutions::new());
        let cleanup_shutdown = Arc::new(AtomicBool::new(false));

        // Start cleanup task for old processes
        let process_cleanup_task =
            Self::start_cleanup_task(process_registry.clone(), cleanup_shutdown.clone());

        Self {
            session_id,
            session_manager,
            isolation_manager: crate::session_isolation::SessionIsolationManager::new(),
            process_registry,
            pending_executions,
            shell_manager: Arc::new(persistent_shell::PersistentShellManager::new()),
            context_cache: Arc::new(tokio::sync::RwLock::new(None)),
            cleanup_shutdown,
            cleanup_tasks: vec![process_cleanup_task],
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
    fn start_cleanup_task(
        registry: terminal_manager::ProcessRegistry,
        shutdown: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            use std::time::Duration;
            let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Every hour

            while !shutdown.load(Ordering::Relaxed) {
                interval.tick().await;
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                Self::cleanup_old_processes(&registry).await;
            }
        })
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
                // Remove cancellation token and completion notifier
                reg.cancellation_tokens.remove(&id);
                reg.completion_notifiers.remove(&id);
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
                // Remove cancellation token and completion notifier
                reg.cancellation_tokens.remove(&id);
                reg.completion_notifiers.remove(&id);

                // Kill running processes
                if let Some(pid) = entry.pid {
                    if terminal_manager::is_active_process_status(&entry.status) {
                        info!("Killing running process {} (PID {})", id, pid);

                        #[cfg(unix)]
                        {
                            // Unix: send SIGTERM
                            use std::process::Command;
                            let mut cmd = Command::new("kill");
                            cmd.arg("-TERM").arg(pid.to_string());
                            crate::utils::env::apply_isolated_env(&mut cmd);
                            let _ = cmd.output();
                        }

                        #[cfg(windows)]
                        {
                            // Windows: use taskkill
                            use std::os::windows::process::CommandExt;
                            use std::process::Command;
                            let mut cmd = Command::new("taskkill");
                            cmd.args(["/PID", &pid.to_string(), "/F"]);
                            crate::utils::env::apply_isolated_env(&mut cmd);
                            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                            let _ = cmd.output();
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

        for pending in self.pending_executions.remove_for_session(session_id) {
            if let Some(response_tx) = pending.response_tx {
                let _ = response_tx.send(PendingShellInputResolution::Cancelled);
            }
        }

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
        Arc::new(SecureFileManager::new_scoped_with_base_dir(workspace_dir))
    }

    async fn get_allowed_absolute_skill_roots(
        &self,
        session_id: &str,
    ) -> Result<Vec<PathBuf>, String> {
        let assistant_id = if let Some(repo) = crate::state::try_get_session_repository() {
            repo.get_session(session_id)
                .await
                .map_err(|e| format!("Failed to load session metadata: {e}"))?
                .and_then(|session| {
                    let config_str = session.agent_config?;
                    let config = serde_json::from_str::<Value>(&config_str).ok()?;
                    config
                        .get("assistantId")
                        .or_else(|| config.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
        } else {
            None
        };

        let workspace_dir = self.get_workspace_dir(session_id);
        let (system_dir, user_dir, assistant_dir, workspace_skill_dir) =
            crate::services::skill_service::resolve_skill_directories(
                assistant_id.as_deref(),
                Some(session_id),
                Some(&workspace_dir),
            )
            .await?;

        Ok(crate::services::skill_service::collect_allowed_skill_roots(
            system_dir,
            user_dir,
            assistant_dir,
            workspace_skill_dir,
        ))
    }

    fn path_is_within_any_root(candidate_path: &Path, allowed_roots: &[PathBuf]) -> bool {
        let normalized_candidate = candidate_path
            .canonicalize()
            .unwrap_or_else(|_| candidate_path.to_path_buf());

        allowed_roots.iter().any(|root| {
            let normalized_root = root.canonicalize().unwrap_or_else(|_| root.clone());
            normalized_candidate.starts_with(&normalized_root)
        })
    }

    pub async fn validate_read_path_with_skill_access(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());
        let file_manager = self.get_file_manager(Some(target_session_id.clone()));

        match file_manager
            .get_security_validator()
            .validate_path_for_read(path_str)
        {
            Ok(path) => Ok(path),
            Err(original_error) => {
                let candidate_path = PathBuf::from(path_str);
                if !candidate_path.is_absolute() {
                    return Err(format!("Security error: {original_error}"));
                }

                let allowed_roots = self
                    .get_allowed_absolute_skill_roots(&target_session_id)
                    .await?;
                if !Self::path_is_within_any_root(&candidate_path, &allowed_roots) {
                    return Err(format!("Security error: {original_error}"));
                }

                let permissive_manager = SecureFileManager::new_with_base_dir(
                    self.get_workspace_dir(&target_session_id),
                );
                permissive_manager
                    .get_security_validator()
                    .validate_path_for_read(path_str)
                    .map_err(|e| format!("Security error: {e}"))
            }
        }
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

    /// Validate path for write/create operations.
    /// Blocks Windows reserved filenames in addition to standard security checks.
    /// Delete operations should use `validate_path_with_error` instead so that
    /// pre-existing reserved-name files can still be cleaned up.
    pub fn validate_path_with_error_for_write(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let file_manager = self.get_file_manager(session_id);
        file_operations::utils::validate_path_with_error_for_write(&file_manager, path_str)
    }

    #[allow(dead_code)]
    fn get_workspace_tree(&self, path: &str, max_depth: usize) -> String {
        use std::fs;

        fn build_tree(
            dir: &std::path::Path,
            prefix: &str,
            depth: usize,
            max_depth: usize,
            workspace_root: &std::path::Path,
        ) -> String {
            if depth >= max_depth {
                return String::new();
            }

            let mut result = String::new();
            if let Ok(entries) = fs::read_dir(dir) {
                let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                entries.retain(|entry| {
                    !utils::is_internal_workspace_artifact_path(workspace_root, &entry.path())
                });
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
                                workspace_root,
                            ));
                        }
                    }
                }
            }
            result
        }

        let workspace_root = std::path::Path::new(path);
        build_tree(workspace_root, "", 0, max_depth, workspace_root)
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

impl Drop for WorkspaceServer {
    fn drop(&mut self) {
        self.cleanup_shutdown.store(true, Ordering::Relaxed);
        for handle in self.cleanup_tasks.drain(..) {
            handle.abort();
        }
    }
}

pub const NAME: &str = "workspace";

#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Integrated workspace for file operations and code execution

Internal paths: .libragent/tmp/ (process outputs), .libragent/exports/ (exported files). These are hidden from listDir/search/export operations to keep user workspace clean. Do not reference them as inputs."
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
                    return ServiceContext::new(cached_prompt.clone())
                        .with_structured_state(json!({
                            "cached": true,
                            "session_id": session_id
                        }))
                        .with_volatility(ContextVolatility::Volatile);
                }
            }
        }

        let workspace_dir_path = self.get_workspace_dir(&session_id);
        let workspace_dir = {
            let path_str = workspace_dir_path.to_string_lossy().to_string();
            #[cfg(target_os = "windows")]
            let path_str = path_str.replace('\\', "/");
            path_str
        };

        // Platform information
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let shell = detect_shell(os);

        // Get current shell CWD
        let shell_cwd = if let Some(cwd) = self.shell_manager.get_shell_cwd(&session_id).await {
            let cwd = {
                #[cfg(target_os = "windows")]
                let cwd = cwd.replace('\\', "/");
                cwd
            };

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
                    let mut running_processes: Vec<(String, String)> = reg
                        .entries
                        .values()
                        .filter(|e| e.session_id == session_id)
                        .filter(|e| terminal_manager::is_active_process_status(&e.status))
                        .map(|e| (e.id.clone(), e.command.clone()))
                        .collect();

                    running_processes.sort_by(|left, right| left.0.cmp(&right.0));
                    let running_count = running_processes.len();
                    let displayed_processes =
                        running_processes.into_iter().take(5).collect::<Vec<_>>();

                    let total_count = reg
                        .entries
                        .values()
                        .filter(|e| e.session_id == session_id)
                        .count();

                    let running_text = if running_count == 0 {
                        "None".to_string()
                    } else {
                        let process_list = displayed_processes
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

### Live State
- Workspace Root: {}
- Persistent Shell CWD: {}
- Running Processes: {}{}
- Internal Paths: `.libragent/tmp/` (process I/O), `.libragent/exports/` (exported files) are hidden from listing to keep workspace clean.
- Total Processes: {}",
            workspace_dir, shell_cwd, running_count, running_processes_text, total_count
        );

        // Update cache
        if let Ok(mut guard) = self.context_cache.try_write() {
            *guard = Some((context_prompt.clone(), std::time::Instant::now()));
        }

        ServiceContext::new(context_prompt)
            .with_structured_state(json!({
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
            }))
            .with_volatility(ContextVolatility::Volatile)
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let logged_args = if tool_name == SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL {
            serde_json::json!({ "redacted": true })
        } else {
            args.clone()
        };
        info!(
            "Workspace tool called: {} with args: {:?}",
            tool_name, logged_args
        );

        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());

        match tool_name {
            // File operation tools
            "readFile" => self.handle_read_file(args, session_id).await,
            "writeFile" => self.handle_write_file(args, session_id).await,
            "listDirectory" => self.handle_list_directory(args, session_id).await,
            "importFiles" => self.handle_import_files(args, session_id).await,
            "search" => self.handle_search(args, session_id).await,
            // editFiles is the model-facing mutation tool. The legacy editFile and per-operation
            // aliases remain dispatchable for backward compatibility and internally normalize
            // into editFiles.
            "editFiles" => self.handle_edit_files(args, session_id).await,
            "editFile" => self.handle_edit_file(args, session_id).await,
            "replaceLines" => self.handle_replace_lines(args, session_id).await,
            "insertAfterLine" => self.handle_insert_after_line(args, session_id).await,
            "deleteLines" => self.handle_delete_lines(args, session_id).await,
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
            // Background process execution (platform-agnostic)
            "spawnProcess" => self.handle_spawn_process(args, &target_session_id).await,
            SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL => {
                self.handle_submit_interactive_shell_input(args, &target_session_id)
                    .await
            }
            CANCEL_INTERACTIVE_SHELL_INPUT_INTERNAL => {
                self.handle_cancel_pending_execution(args, &target_session_id)
                    .await
            }
            // Export tools
            "export" => self.handle_export(args, session_id).await,
            // Terminal/Process management tools
            "readProcessOutput" => {
                self.handle_read_process_output(args, &target_session_id)
                    .await
            }
            "listProcesses" => self.handle_list_processes(args, &target_session_id).await,
            "stopProcess" => self.handle_stop_process(args, &target_session_id).await,
            "waitForProcess" => self.handle_wait_for_process(args, &target_session_id).await,
            // Backward-compat alias: pollProcess was the old name for non-blocking status check.
            // Always inject timeout=0 so semantics are preserved.
            "pollProcess" => {
                let mut poll_args = args.clone();
                poll_args["timeout"] = serde_json::json!(0);
                self.handle_wait_for_process(poll_args, &target_session_id)
                    .await
            }

            _ => Err(format!("Tool '{tool_name}' not found")),
        }
        .or_else(|e| {
            if e.contains("cancelled") || e.contains("interrupted") {
                return Err(e);
            }
            Ok(guided_error(ErrorCategory::InternalError, e, ToolGroup::Workspace).to_mcp_result())
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_executions_cleanup() {
        let pending = PendingExecutions::new();
        let now = chrono::Utc::now();

        // Add one old entry (15 minutes ago)
        pending.insert(PendingShellExecution {
            execution_id: "old".to_string(),
            session_id: "sess".to_string(),
            executable_command: "ls".to_string(),
            display_command: "ls".to_string(),
            run_mode: "sync".to_string(),
            timeout: 30,
            created_at: now - chrono::Duration::minutes(15),
            prompt: "prompt".to_string(),
            input_type: InteractiveShellInputType::Text,
            response_tx: None,
        });

        // Add one new entry
        pending.insert(PendingShellExecution {
            execution_id: "new".to_string(),
            session_id: "sess".to_string(),
            executable_command: "ls".to_string(),
            display_command: "ls".to_string(),
            run_mode: "sync".to_string(),
            timeout: 30,
            created_at: now,
            prompt: "prompt".to_string(),
            input_type: InteractiveShellInputType::Text,
            response_tx: None,
        });

        assert_eq!(pending.count(), 2);

        // Cleanup entries older than 10 minutes (600s)
        pending.cleanup_expired(600);

        assert_eq!(pending.count(), 1);
        assert!(pending.remove("new").is_some());
        assert!(pending.remove("old").is_none());
    }
}
