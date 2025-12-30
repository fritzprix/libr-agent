use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::info;

use super::BuiltinMCPServer;
use crate::mcp::types::{MCPResult, ServiceContext, ServiceContextOptions};
use crate::mcp::MCPTool;
use crate::services::SecureFileManager;
use crate::session::SessionManager;

// Module imports
pub mod code_execution;
pub mod export_operations;
pub mod file_operations;
pub mod persistent_shell;
pub mod persistent_shell_manager;
pub mod terminal_manager;
pub mod tools;
pub mod ui_resources;
pub mod utils;

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
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) isolation_manager: crate::session_isolation::SessionIsolationManager,
    pub(crate) process_registry: terminal_manager::ProcessRegistry,
    pub(crate) pending_executions: Arc<PendingExecutions>,
    pub(crate) shell_manager: Arc<persistent_shell_manager::PersistentShellManager>,
}

impl WorkspaceServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        info!("WorkspaceServer using session-based workspace management");
        let process_registry = terminal_manager::create_process_registry();

        // Start cleanup task for old processes
        Self::start_cleanup_task(process_registry.clone());

        Self {
            session_manager,
            isolation_manager: crate::session_isolation::SessionIsolationManager::new(),
            process_registry,
            pending_executions: Arc::new(PendingExecutions::new()),
            shell_manager: Arc::new(persistent_shell_manager::PersistentShellManager::new()),
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

    // Terminal Tool Handlers

    /// Handle poll_process tool call
    pub async fn handle_poll_process(&self, args: Value) -> Result<MCPResult, String> {
        // Parse processId
        let process_id = match args.get("processId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Err("Missing required parameter: processId".to_string());
            }
        };

        // Get current session
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Verify session access BEFORE write lock (optimization)
        {
            let registry = self.process_registry.read().await;
            match registry.entries.get(process_id) {
                Some(entry) if entry.session_id == session_id => {
                    // Access granted, continue
                }
                _ => {
                    return Err("Process not found or access denied".to_string());
                }
            }
        }

        // Update poll tracking and get entry + streaming handle (write lock)
        let threshold = crate::config::poll_threshold();
        let (should_show_guidance, entry_for_response, streaming_handle) = {
            let mut registry = self.process_registry.write().await;
            let entry_clone = if let Some(entry) = registry.entries.get_mut(process_id) {
                let now = chrono::Utc::now();

                // Update poll metadata
                entry.last_poll_at = Some(now);
                entry.poll_count += 1;

                // Track consecutive running polls
                let is_running = matches!(entry.status, terminal_manager::ProcessStatus::Running);
                if is_running {
                    if entry.first_running_poll_at.is_none() {
                        entry.first_running_poll_at = Some(now);
                    }
                    entry.consecutive_running_polls += 1;
                } else {
                    // Reset counters when status changes from running
                    entry.consecutive_running_polls = 0;
                    entry.first_running_poll_at = None;
                }

                let should_guide = is_running && entry.consecutive_running_polls >= threshold;
                (should_guide, entry.clone())
            } else {
                return Err("Process not found or access denied".to_string());
            };

            // Get streaming handle after releasing entry borrow
            let handle = registry.streaming_handles.get(process_id).cloned();
            (entry_clone.0, entry_clone.1, handle)
        };

        // Build response
        let mut response = serde_json::json!({
            "process_id": entry_for_response.id,
            "status": format!("{:?}", entry_for_response.status).to_lowercase(),
            "command": entry_for_response.command,
            "pid": entry_for_response.pid,
            "exit_code": entry_for_response.exit_code,
            "started_at": entry_for_response.started_at.to_rfc3339(),
            "finished_at": entry_for_response.finished_at.map(|t| t.to_rfc3339()),
            "stdout_size": entry_for_response.stdout_size,
            "stderr_size": entry_for_response.stderr_size,
            "streaming_available": streaming_handle.is_some(),
        });

        // Optional tail - check in-memory buffer first, fallback to file
        if let Some(tail_obj) = args.get("tail").and_then(|v| v.as_object()) {
            let src = tail_obj
                .get("src")
                .and_then(|v| v.as_str())
                .unwrap_or("stdout");
            let n = tail_obj.get("n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

            let (lines, source) = if let Some(handle) = &streaming_handle {
                // Fast path: get from in-memory buffer
                let stream_type = if src == "stdout" {
                    terminal_manager::StreamType::Stdout
                } else {
                    terminal_manager::StreamType::Stderr
                };
                (handle.get_tail(stream_type, n).await, "buffer")
            } else {
                // Fallback: read from file (process finished or old entry)
                let file_path = if src == "stdout" {
                    std::path::PathBuf::from(&entry_for_response.stdout_path)
                } else {
                    std::path::PathBuf::from(&entry_for_response.stderr_path)
                };
                match terminal_manager::tail_lines(&file_path, n).await {
                    Ok(lines) => (lines, "file"),
                    Err(e) => {
                        tracing::warn!("Failed to read tail from file: {}", e);
                        (Vec::new(), "error")
                    }
                }
            };

            response["tail"] = serde_json::json!({
                "src": src,
                "lines": lines,
                "source": source,
            });
        }

        // Add guidance message if threshold exceeded
        let response_text = if should_show_guidance {
            let guidance = format!(
                "\n\n[POLLING GUIDANCE]\n\
                Process status: RUNNING\n\
                Polls detected: {} consecutive checks\n\
                \n\
                RECOMMENDED ACTION:\n\
                - Wait at least 10 seconds before next poll\n\
                - Process will continue running in background\n\
                - Status will update automatically when complete\n\
                \n\
                ALTERNATIVE: Use async wait patterns instead of active polling.\n\
                Frequent polling may impact system performance without providing additional value.",
                entry_for_response.consecutive_running_polls
            );

            format!(
                "{}\n{}",
                serde_json::to_string_pretty(&response).unwrap_or_default(),
                guidance
            )
        } else {
            serde_json::to_string_pretty(&response).unwrap_or_default()
        };

        Ok(MCPResult::success(&response_text))
    }

    /// Handle read_process_output tool call
    pub async fn handle_read_process_output(&self, args: Value) -> Result<MCPResult, String> {
        // Parse parameters
        let process_id = match args.get("processId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Err("Missing required parameter: processId".to_string());
            }
        };

        let stream = match args.get("stream").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return Err("Missing required parameter: stream".to_string());
            }
        };

        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("tail");

        let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        // Get current session
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Get process entry
        let registry = self.process_registry.read().await;
        let entry = match registry.entries.get(process_id) {
            Some(e) => e.clone(),
            None => {
                return Err("Process not found or access denied".to_string());
            }
        };

        // Verify session access
        if entry.session_id != session_id {
            return Err("Process not found or access denied".to_string());
        }
        drop(registry);

        // Get file path
        let file_path = if stream == "stdout" {
            std::path::PathBuf::from(&entry.stdout_path)
        } else {
            std::path::PathBuf::from(&entry.stderr_path)
        };

        // Read lines based on mode
        let content = match mode {
            "head" => terminal_manager::head_lines(&file_path, lines).await,
            _ => terminal_manager::tail_lines(&file_path, lines).await,
        };

        match content {
            Ok(lines_vec) => {
                let response = serde_json::json!({
                    "process_id": process_id,
                    "stream": stream,
                    "mode": mode,
                    "lines_requested": lines.min(100),
                    "lines_returned": lines_vec.len(),
                    "content": lines_vec,
                    "total_size": terminal_manager::get_file_size(&file_path).await,
                    "note": "Text output only. Max 100 lines per request.",
                });

                Ok(MCPResult::success(
                    &serde_json::to_string_pretty(&response).unwrap_or_default(),
                ))
            }
            Err(e) => Err(format!("Failed to read output: {e}")),
        }
    }

    /// Handle list_processes tool call
    pub async fn handle_list_processes(&self, args: Value) -> Result<MCPResult, String> {
        let status_filter = args
            .get("statusFilter")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        // Get current session
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Filter processes by session
        let registry = self.process_registry.read().await;
        let mut processes: Vec<Value> = registry
            .entries
            .values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| match status_filter {
                "running" => matches!(e.status, terminal_manager::ProcessStatus::Running),
                "finished" => matches!(
                    e.status,
                    terminal_manager::ProcessStatus::Finished
                        | terminal_manager::ProcessStatus::Failed
                ),
                _ => true,
            })
            .map(|e| {
                serde_json::json!({
                    "process_id": e.id,
                    "command": e.command,
                    "status": format!("{:?}", e.status).to_lowercase(),
                    "pid": e.pid,
                    "started_at": e.started_at.to_rfc3339(),
                    "exit_code": e.exit_code,
                })
            })
            .collect();

        processes.sort_by(|a, b| {
            let a_time = a.get("started_at").and_then(|v| v.as_str()).unwrap_or("");
            let b_time = b.get("started_at").and_then(|v| v.as_str()).unwrap_or("");
            b_time.cmp(a_time) // descending order
        });

        let total = processes.len();
        let running = registry
            .entries
            .values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| matches!(e.status, terminal_manager::ProcessStatus::Running))
            .count();
        let finished = registry
            .entries
            .values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| {
                matches!(
                    e.status,
                    terminal_manager::ProcessStatus::Finished
                        | terminal_manager::ProcessStatus::Failed
                )
            })
            .count();

        drop(registry);

        let response = serde_json::json!({
            "processes": processes,
            "total": total,
            "running": running,
            "finished": finished,
        });

        Ok(MCPResult::success(
            &serde_json::to_string_pretty(&response).unwrap_or_default(),
        ))
    }

    /// Handle stop_process tool call
    pub async fn handle_stop_process(&self, args: Value) -> Result<MCPResult, String> {
        let process_id = match args.get("processId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Err("Missing required parameter: processId".to_string());
            }
        };

        // Get current session
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let mut registry = self.process_registry.write().await;

        // Check if process exists and belongs to session
        if let Some(entry) = registry.entries.get(process_id) {
            if entry.session_id != session_id {
                return Err("Process not found or access denied".to_string());
            }
        } else {
            return Err("Process not found".to_string());
        }

        // Cancel process via token
        if let Some(token) = registry.cancellation_tokens.get(process_id) {
            token.cancel();
        }

        // Update status and kill process
        if let Some(entry) = registry.entries.get_mut(process_id) {
            // Kill process if running
            if let Some(pid) = entry.pid {
                if matches!(
                    entry.status,
                    terminal_manager::ProcessStatus::Running
                        | terminal_manager::ProcessStatus::Starting
                ) {
                    info!("Force-killing process {} (PID {})", process_id, pid);

                    #[cfg(unix)]
                    {
                        use std::process::Command;
                        let _ = Command::new("kill")
                            .arg("-TERM")
                            .arg(pid.to_string())
                            .output();
                    }

                    #[cfg(windows)]
                    {
                        use std::process::Command;
                        let _ = Command::new("taskkill")
                            .args(["/PID", &pid.to_string(), "/F"])
                            .output();
                    }
                }
            }

            entry.status = terminal_manager::ProcessStatus::Killed;
            entry.finished_at = Some(chrono::Utc::now());
        }

        // Remove cancellation token
        registry.cancellation_tokens.remove(process_id);

        Ok(MCPResult::success(&format!(
            "Process {} stopped",
            process_id
        )))
    }

    // Common utility methods
    pub fn get_workspace_dir(&self) -> std::path::PathBuf {
        self.session_manager.get_session_workspace_dir()
    }

    pub fn get_file_manager(&self) -> Arc<SecureFileManager> {
        self.session_manager.get_file_manager()
    }

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
        let mut tools = Vec::new();
        tools.extend(tools::file_tools());
        tools.extend(tools::code_tools());
        tools.extend(tools::export_tools());
        tools.extend(tools::terminal_tools());
        tools
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Get session-specific workspace directory
        let workspace_dir_path = self.get_workspace_dir();
        let workspace_dir = workspace_dir_path.to_string_lossy().to_string();

        // Generate directory tree (2 levels deep)
        let tree_output = self.get_workspace_tree(&workspace_dir, 2);

        // Get running process count
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Try to get running count without blocking
        // If we can't acquire the lock immediately, return 0 to avoid blocking
        let running_count = {
            match self.process_registry.try_read() {
                Ok(reg) => reg
                    .entries
                    .values()
                    .filter(|e| e.session_id == session_id)
                    .filter(|e| matches!(e.status, terminal_manager::ProcessStatus::Running))
                    .count(),
                Err(_) => {
                    // Lock is held by another task, return 0 to avoid blocking
                    tracing::debug!("Could not acquire process registry lock for service context");
                    0
                }
            }
        };

        // Platform information
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        info!(
            "Workspace service context - workspace_dir: {}, tree_output length: {}, running processes: {}, platform: {}/{}",
            workspace_dir,
            tree_output.len(),
            running_count,
            os,
            arch
        );

        let context_prompt = format!(
            "## Workspace\n\n**Directory**: {}\n**Running Processes**: {}\n**Platform**: {}/{}",
            workspace_dir, running_count, os, arch
        );

        ServiceContext {
            context_prompt,
            structured_state: Some(json!({
                "workspace_dir": workspace_dir,
                "workspace_tree": tree_output,
                "platform": {
                    "os": os,
                    "arch": arch
                },
                "running_processes": running_count,
                "tools_count": self.tools().len()
            })),
        }
    }

    async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> {
        // Update session context if session_id is provided
        if let Some(new_session_id) = options.session_id {
            info!("Switching workspace context to session: {}", new_session_id);

            // Get current session before switching
            let old_session_id = self
                .session_manager
                .get_current_session()
                .unwrap_or_else(|| "default".to_string());

            info!(
                "Checking context switch: old='{}', new='{}'",
                old_session_id, new_session_id
            );

            // Cancel all processes for the old session
            if old_session_id != new_session_id {
                info!(
                    "Switching workspace context: {} -> {}",
                    old_session_id, new_session_id
                );

                info!("Acquiring process registry lock for cleanup...");
                let mut reg = self.process_registry.write().await;
                info!("Process registry lock acquired. Starting cleanup.");

                // Get all process IDs for the old session
                let old_session_processes: Vec<String> = reg
                    .entries
                    .values()
                    .filter(|e| e.session_id == old_session_id)
                    .filter(|e| {
                        matches!(
                            e.status,
                            terminal_manager::ProcessStatus::Starting
                                | terminal_manager::ProcessStatus::Running
                        )
                    })
                    .map(|e| e.id.clone())
                    .collect();

                // Cancel all processes via their tokens
                for process_id in &old_session_processes {
                    if let Some(token) = reg.cancellation_tokens.get(process_id) {
                        info!("Cancelling process: {}", process_id);
                        token.cancel();
                    }

                    // Update status to Killed
                    if let Some(entry) = reg.entries.get_mut(process_id) {
                        entry.status = terminal_manager::ProcessStatus::Killed;
                        entry.finished_at = Some(chrono::Utc::now());
                    }
                }

                // Also kill by PID for safety (in case token didn't work)
                for process_id in old_session_processes {
                    if let Some(entry) = reg.entries.get(&process_id) {
                        if let Some(pid) = entry.pid {
                            info!("Force-killing process {} (PID {})", process_id, pid);

                            #[cfg(unix)]
                            {
                                use std::process::Command;
                                let _ = Command::new("kill")
                                    .arg("-TERM")
                                    .arg(pid.to_string())
                                    .output();
                            }

                            #[cfg(windows)]
                            {
                                use std::process::Command;
                                let _ = Command::new("taskkill")
                                    .args(["/PID", &pid.to_string(), "/F"])
                                    .output();
                            }
                        }
                    }
                }

                drop(reg);
                info!("Process registry lock released.");
            } else {
                info!("Session context unchanged, skipping process cleanup.");
            }

            // Switch session in session_manager (use async version to avoid blocking)
            info!("Updating session manager context to: {}", new_session_id);
            if let Err(e) = self
                .session_manager
                .set_session_async(new_session_id.clone())
                .await
            {
                return Err(format!("Failed to switch session in session_manager: {e}"));
            }
            info!("Session manager context updated successfully.");

            // The session manager handles session-specific workspace directories
            // No additional action needed as get_workspace_dir() uses session context
        }

        // Update assistant context if assistant_id is provided
        if let Some(assistant_id) = options.assistant_id {
            info!("Switching workspace context to assistant: {}", assistant_id);
            // Workspace server doesn't filter by assistant, but logs for awareness
        }

        Ok(())
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        info!("Workspace tool called: {} with args: {:?}", tool_name, args);
        match tool_name {
            // File operation tools
            "readFile" => self.handle_read_file(args).await,
            "writeFile" => self.handle_write_file(args).await,
            "listDirectory" => self.handle_list_directory(args).await,
            "replaceLinesInFile" => self.handle_replace_lines_in_file(args).await,
            "importFile" => self.handle_import_file(args).await,
            // Code execution tools
            // Note: Python/TypeScript execution were removed from the public tool
            // interface to avoid external runtime dependencies and to prevent
            // agents from controlling isolation/permissions. Only shell
            // execution remains exposed below.
            // Platform-specific shell execution tools
            #[cfg(unix)]
            "executeShell" => self.handle_execute_shell(args).await,
            #[cfg(windows)]
            "executeWindowsCmd" => self.handle_execute_shell(args).await,
            // Interactive shell execution (2nd tool for user input)
            "executePendingShell" => self.handle_execute_pending_shell(args).await,
            // Cancel pending execution (UI callback tool)
            "cancelPendingExecution" => self.handle_cancel_pending_execution(args).await,
            // Export tools
            "exportFile" => self.handle_export_file(args).await,
            "exportZip" => self.handle_export_zip(args).await,
            // Terminal/Process management tools
            "pollProcess" => self.handle_poll_process(args).await,
            "readProcessOutput" => self.handle_read_process_output(args).await,
            "listProcesses" => self.handle_list_processes(args).await,
            "stopProcess" => self.handle_stop_process(args).await,

            // --- Error Hints for Common Mistakes ---
            "read_file" | "readContent" => Ok(MCPResult::error(
                "Tool not found. Did you mean 'readFile'? Please use the exact tool name 'readFile'."
            )),
            "write_file" | "writeContent" => Ok(MCPResult::error(
                "Tool not found. Did you mean 'writeFile'? Please use the exact tool name 'writeFile'."
            )),
            "list_directory" | "ls" => Ok(MCPResult::error(
                "Tool not found. Did you mean 'listDirectory'? Please use the exact tool name 'listDirectory'."
            )),
            "execute_shell" | "execute_command" => Ok(MCPResult::error(
                "Tool not found. Did you mean 'executeShell'? Please use the exact tool name 'executeShell'."
            )),
            "execute_windows_cmd" => Ok(MCPResult::error(
                "Tool not found. Did you mean 'executeWindowsCmd'? Please use the exact tool name 'executeWindowsCmd'."
            )),
            _ => Err(format!("Tool '{tool_name}' not found")),
        }
    }
}
