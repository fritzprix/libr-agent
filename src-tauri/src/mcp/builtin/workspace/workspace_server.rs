use super::persistent_shell;
use super::terminal_manager;
use super::tools;
use super::types::{PendingExecutions, PendingShellInputResolution};
use super::utils;
use crate::mcp::builtin::utils::path_starts_with;
use crate::mcp::MCPTool;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::SessionRepository;
use crate::session::SessionManager;
use crate::session_isolation::PathMappingLayer;
use crate::SecureFileManager;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

#[derive(Debug)]
pub struct WorkspaceServer {
    pub(crate) session_id: String,
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) isolation_manager: crate::session_isolation::SessionIsolationManager,
    pub(crate) process_registry: terminal_manager::ProcessRegistry,
    pub(crate) pending_executions: Arc<PendingExecutions>,
    pub(crate) shell_manager: Arc<persistent_shell::PersistentShellManager>,
    pub(crate) context_cache: Arc<tokio::sync::RwLock<Option<(String, std::time::Instant)>>>,
    cleanup_shutdown: Arc<std::sync::atomic::AtomicBool>,
    cleanup_tasks: Vec<JoinHandle<()>>,
}

const TEAMWORK_ALIAS_PREFIX: &str = "@teamwork";
// Guard against pathological or cyclic parent-session chains while still
// allowing deep enough org hierarchies for normal teamwork lineages.
const TEAMWORK_PARENT_CHAIN_LIMIT: usize = 64;

impl WorkspaceServer {
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        info!("WorkspaceServer created for session: {}", session_id);
        let process_registry = terminal_manager::create_process_registry();
        let pending_executions = Arc::new(PendingExecutions::new());
        let cleanup_shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

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
        shutdown: Arc<std::sync::atomic::AtomicBool>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            use std::time::Duration;
            let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Every hour

            while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                interval.tick().await;
                if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
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
                .and_then(|session| crate::agent::extract_assistant_id_from_session(&session))
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

    async fn get_skill_alias_roots(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::services::skill_service::SkillAliasRoot>, String> {
        let assistant_id = if let Some(repo) = crate::state::try_get_session_repository() {
            repo.get_session(session_id)
                .await
                .map_err(|e| format!("Failed to load session metadata: {e}"))?
                .and_then(|session| crate::agent::extract_assistant_id_from_session(&session))
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

        Ok(crate::services::skill_service::collect_skill_alias_roots(
            system_dir,
            user_dir,
            assistant_dir,
            workspace_skill_dir,
        ))
    }

    fn extract_teamwork_alias_relative_path(path_str: &str) -> Option<&str> {
        if path_str == TEAMWORK_ALIAS_PREFIX
            || path_str == ".libragent/teamwork"
            || path_str == ".libragent\\teamwork"
        {
            return Some(".");
        }

        path_str
            .strip_prefix("@teamwork/")
            .or_else(|| path_str.strip_prefix("@teamwork\\"))
            .or_else(|| path_str.strip_prefix(".libragent/teamwork/"))
            .or_else(|| path_str.strip_prefix(".libragent\\teamwork\\"))
            .map(|suffix| {
                if suffix.trim().is_empty() {
                    "."
                } else {
                    suffix
                }
            })
    }

    async fn resolve_teamwork_root_session_id(&self, session_id: &str) -> Result<String, String> {
        let Some(repo) = crate::state::try_get_session_repository() else {
            return Ok(session_id.to_string());
        };

        let mut current = match repo
            .get_session(session_id)
            .await
            .map_err(|e| format!("Failed to load session metadata: {e}"))?
        {
            Some(session) => session,
            None => return Ok(session_id.to_string()),
        };

        if let Some(org_root_session_id) = current.org_root_session_id.clone() {
            return Ok(org_root_session_id);
        }

        for _ in 0..TEAMWORK_PARENT_CHAIN_LIMIT {
            let Some(parent_session_id) = current.parent_session_id.clone() else {
                return Ok(current.id);
            };

            current = match repo
                .get_session(&parent_session_id)
                .await
                .map_err(|e| format!("Failed to load parent session metadata: {e}"))?
            {
                Some(session) => session,
                None => return Ok(parent_session_id),
            };

            if let Some(org_root_session_id) = current.org_root_session_id.clone() {
                return Ok(org_root_session_id);
            }
        }

        Err(format!(
            "Failed to resolve teamwork root for session {}: parent chain exceeded {} hops or contains a cycle",
            session_id, TEAMWORK_PARENT_CHAIN_LIMIT
        ))
    }

    async fn get_teamwork_artifact_root(&self, session_id: &str) -> Result<PathBuf, String> {
        let root_session_id = self.resolve_teamwork_root_session_id(session_id).await?;
        Ok(crate::session::teamwork_artifact_dir_for_session(
            &self.session_manager,
            &root_session_id,
        ))
    }

    fn path_is_within_any_root(candidate_path: &Path, allowed_roots: &[PathBuf]) -> bool {
        let normalized_candidate = candidate_path
            .canonicalize()
            .unwrap_or_else(|_| candidate_path.to_path_buf());

        allowed_roots.iter().any(|root| {
            let normalized_root = root.canonicalize().unwrap_or_else(|_| root.clone());
            path_starts_with(&normalized_candidate, &normalized_root)
        })
    }

    pub async fn validate_read_path_with_skill_access(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());

        if let Some(teamwork_relative_path) = Self::extract_teamwork_alias_relative_path(path_str) {
            let teamwork_root = self.get_teamwork_artifact_root(&target_session_id).await?;
            let teamwork_manager = SecureFileManager::new_scoped_with_base_dir(teamwork_root);
            return teamwork_manager
                .get_security_validator()
                .validate_path_for_read(teamwork_relative_path)
                .map_err(|e| format!("Security error: {e}"));
        }

        if let Some((alias_prefix, relative_path)) =
            crate::services::skill_service::extract_skill_alias_relative_path(path_str)
        {
            let alias_roots = self.get_skill_alias_roots(&target_session_id).await?;
            let alias_root = alias_roots
                .into_iter()
                .find(|root| root.prefix == alias_prefix)
                .ok_or_else(|| format!("Skill alias root is not available: {alias_prefix}"))?;
            let alias_manager = SecureFileManager::new_scoped_with_base_dir(alias_root.root);
            return alias_manager
                .get_security_validator()
                .validate_path_for_read(relative_path)
                .map_err(|e| format!("Security error: {e}"));
        }

        let mapped_path = self
            .map_docker_container_file_tool_path(path_str, &target_session_id)
            .await?;
        let effective_path = mapped_path.as_deref().unwrap_or(path_str);
        let file_manager = self.get_file_manager(Some(target_session_id.clone()));

        match file_manager
            .get_security_validator()
            .validate_path_for_read(effective_path)
        {
            Ok(path) => Ok(path),
            Err(original_error) => {
                let candidate_path = PathBuf::from(effective_path);
                if !candidate_path.is_absolute() {
                    return Err(format!("Security error: {original_error}"));
                }

                let allowed_roots = self
                    .get_allowed_absolute_skill_roots(&target_session_id)
                    .await?;
                let teamwork_root = self.get_teamwork_artifact_root(&target_session_id).await?;
                let mut allowed_roots = allowed_roots;
                allowed_roots.push(teamwork_root);

                if !Self::path_is_within_any_root(&candidate_path, &allowed_roots) {
                    return Err(format!("Security error: {original_error}"));
                }

                let permissive_manager = SecureFileManager::new_with_base_dir(
                    self.get_workspace_dir(&target_session_id),
                );
                permissive_manager
                    .get_security_validator()
                    .validate_path_for_read(effective_path)
                    .map_err(|e| format!("Security error: {e}"))
            }
        }
    }

    pub async fn validate_write_path_with_teamwork_access(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());

        if let Some(teamwork_relative_path) = Self::extract_teamwork_alias_relative_path(path_str) {
            let teamwork_root = self.get_teamwork_artifact_root(&target_session_id).await?;
            let teamwork_manager = SecureFileManager::new_scoped_with_base_dir(teamwork_root);
            return teamwork_manager
                .get_security_validator()
                .validate_path_for_write(teamwork_relative_path)
                .map_err(|e| format!("Security error: {e}"));
        }

        let mapped_path = self
            .map_docker_container_file_tool_path(path_str, &target_session_id)
            .await?;
        let effective_path = mapped_path.as_deref().unwrap_or(path_str);

        self.validate_path_with_error_for_write(effective_path, Some(target_session_id))
    }

    async fn map_docker_container_file_tool_path(
        &self,
        path_str: &str,
        target_session_id: &str,
    ) -> Result<Option<String>, String> {
        if !path_str.starts_with('/') {
            return Ok(None);
        }

        let Some(session_repo) = crate::state::try_get_session_repository() else {
            return Ok(None);
        };
        let Some(session) = session_repo
            .get_session(target_session_id)
            .await
            .map_err(|e| format!("Failed to load session isolation metadata: {e}"))?
        else {
            return Ok(None);
        };

        if session.workspace_isolation != WorkspaceIsolationMode::Docker {
            return Ok(None);
        }

        let host_workspace = session.docker_host_workspace_path.as_ref().ok_or_else(|| {
            format!("Missing Docker host workspace path for session {target_session_id}")
        })?;
        let mapper = PathMappingLayer::new(PathBuf::from(host_workspace));
        let Some(host_path) = mapper.container_to_host(path_str) else {
            return Err(format!(
                "Docker container path '{path_str}' is outside /workspace. Shell commands may access it, but workspace file tools only map /workspace paths to the host workspace."
            ));
        };

        Ok(Some(host_path.to_string_lossy().to_string()))
    }

    /// Validate path with security checks (helper for file operations)
    pub fn validate_path_with_error(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<std::path::PathBuf, String> {
        let file_manager = self.get_file_manager(session_id);
        super::file_operations::utils::validate_path_with_error(&file_manager, path_str)
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
        super::file_operations::utils::validate_path_with_error_for_write(&file_manager, path_str)
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

    pub async fn get_service_context_internal(
        &self,
        options: Option<&Value>,
    ) -> crate::mcp::types::ServiceContext {
        use super::context;
        use crate::mcp::types::{ContextVolatility, ServiceContext};

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
                        .with_structured_state(serde_json::json!({
                            "cached": true,
                            "session_id": session_id
                        }))
                        .with_volatility(ContextVolatility::Volatile);
                }
            }
        }

        let context_prompt = context::build_context_prompt(
            &session_id,
            &self.session_manager,
            &self.process_registry,
            &self.shell_manager,
        )
        .await;

        // Update cache
        if let Ok(mut guard) = self.context_cache.try_write() {
            *guard = Some((context_prompt.clone(), std::time::Instant::now()));
        }

        let live_state = context::build_workspace_live_state(
            &session_id,
            &self.session_manager,
            &self.shell_manager,
        )
        .await;

        let workspace_dir = live_state.workspace_dir;
        let shell_cwd = live_state.shell_cwd;

        let (running_count, total_count) = match self.process_registry.try_read() {
            Ok(reg) => {
                let running = reg
                    .entries
                    .values()
                    .filter(|e| e.session_id == session_id)
                    .filter(|e| super::terminal_manager::is_active_process_status(&e.status))
                    .count();
                let total = reg
                    .entries
                    .values()
                    .filter(|e| e.session_id == session_id)
                    .count();
                (running, total)
            }
            Err(_) => (0, 0),
        };

        ServiceContext::new(context_prompt)
            .with_structured_state(serde_json::json!({
                "workspace_dir": workspace_dir,
                "shell_cwd": shell_cwd,
                "is_docker": live_state.is_docker,
                "platform": {
                    "os": std::env::consts::OS,
                    "arch": std::env::consts::ARCH,
                    "shell": context::detect_shell(std::env::consts::OS)
                },
                "processes": {
                    "running": running_count,
                    "total": total_count,
                },
                "shell_active": !shell_cwd.is_empty(),
                "tools_count": Self::tools_static().len()
            }))
            .with_volatility(ContextVolatility::Volatile)
    }
}

impl Drop for WorkspaceServer {
    fn drop(&mut self) {
        self.cleanup_shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        for handle in self.cleanup_tasks.drain(..) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_teamwork_alias_relative_path() {
        // Test @teamwork
        assert_eq!(
            WorkspaceServer::extract_teamwork_alias_relative_path("@teamwork"),
            Some(".")
        );
        assert_eq!(
            WorkspaceServer::extract_teamwork_alias_relative_path(
                "@teamwork/coordination/KANBAN.md"
            ),
            Some("coordination/KANBAN.md")
        );
        assert_eq!(
            WorkspaceServer::extract_teamwork_alias_relative_path(
                "@teamwork\\coordination\\KANBAN.md"
            ),
            Some("coordination\\KANBAN.md")
        );

        // Test .libragent/teamwork
        assert_eq!(
            WorkspaceServer::extract_teamwork_alias_relative_path(".libragent/teamwork"),
            Some(".")
        );
        assert_eq!(
            WorkspaceServer::extract_teamwork_alias_relative_path(
                ".libragent/teamwork/coordination/KANBAN.md"
            ),
            Some("coordination/KANBAN.md")
        );
        assert_eq!(
            WorkspaceServer::extract_teamwork_alias_relative_path(
                ".libragent\\teamwork\\coordination\\KANBAN.md"
            ),
            Some("coordination\\KANBAN.md")
        );

        // Test standard files (should return None)
        assert_eq!(
            WorkspaceServer::extract_teamwork_alias_relative_path("src/main.rs"),
            None
        );
        assert_eq!(
            WorkspaceServer::extract_teamwork_alias_relative_path("docs/README.md"),
            None
        );
    }
}
