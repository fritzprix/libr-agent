mod lifecycle;
mod path_validation;
mod service_context;
mod teamwork_paths;

use super::persistent_shell;
use super::terminal_manager;
use super::tools;
use super::types::PendingExecutions;
use super::utils;
use crate::mcp::MCPTool;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::session::SessionManager;
use crate::SecureFileManager;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

/// Cached workspace service-context prompt text (TTL-managed in get_service_context).
pub(crate) type ContextCache = Arc<tokio::sync::RwLock<Option<(String, std::time::Instant)>>>;

/// Clear the workspace context cache. Safe to call from background process tasks.
pub(crate) async fn clear_context_cache(cache: &ContextCache) {
    *cache.write().await = None;
    tracing::debug!("Workspace service context cache cleared");
}

#[derive(Debug)]
pub struct WorkspaceServer {
    pub(crate) session_id: String,
    pub(crate) session_manager: Arc<SessionManager>,
    /// Fixed at session create; drives which shell tools are discoverable.
    pub(crate) workspace_isolation: WorkspaceIsolationMode,
    pub(crate) isolation_manager: crate::session_isolation::SessionIsolationManager,
    pub(crate) process_registry: terminal_manager::ProcessRegistry,
    pub(crate) pending_executions: Arc<PendingExecutions>,
    pub(crate) shell_manager: Arc<persistent_shell::PersistentShellManager>,
    pub(crate) context_cache: ContextCache,
    cleanup_shutdown: Arc<std::sync::atomic::AtomicBool>,
    cleanup_tasks: Vec<JoinHandle<()>>,
}

impl WorkspaceServer {
    /// Create a workspace server for host isolation (tests / legacy global registry).
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        Self::with_isolation(session_id, session_manager, WorkspaceIsolationMode::Host)
    }

    /// Create a workspace server bound to a session's workspace isolation mode.
    pub fn with_isolation(
        session_id: String,
        session_manager: Arc<SessionManager>,
        workspace_isolation: WorkspaceIsolationMode,
    ) -> Self {
        info!(
            "WorkspaceServer created for session: {} (isolation={})",
            session_id, workspace_isolation
        );
        let process_registry = terminal_manager::create_process_registry();
        let pending_executions = Arc::new(PendingExecutions::new());
        let cleanup_shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let process_cleanup_task =
            lifecycle::start_cleanup_task(process_registry.clone(), cleanup_shutdown.clone());

        Self {
            session_id,
            session_manager,
            workspace_isolation,
            isolation_manager: crate::session_isolation::SessionIsolationManager::new(),
            process_registry,
            pending_executions,
            shell_manager: Arc::new(persistent_shell::PersistentShellManager::new()),
            context_cache: Arc::new(tokio::sync::RwLock::new(None)),
            cleanup_shutdown,
            cleanup_tasks: vec![process_cleanup_task],
        }
    }

    pub(crate) fn code_tools_profile(&self) -> tools::CodeToolsProfile {
        tools::CodeToolsProfile::from_isolation(self.workspace_isolation)
    }

    /// Invalidate the service context cache (call after state changes)
    pub(crate) async fn invalidate_context_cache(&self) {
        clear_context_cache(&self.context_cache).await;
    }

    pub fn get_workspace_dir(&self, session_id: &str) -> std::path::PathBuf {
        self.session_manager
            .get_session_workspace_dir_by_id(session_id)
    }

    pub fn get_file_manager(&self, session_id: Option<String>) -> Arc<SecureFileManager> {
        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());
        let workspace_dir = self.get_workspace_dir(&target_session_id);
        Arc::new(SecureFileManager::new_scoped_with_base_dir(workspace_dir))
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

    /// Host-platform tool definitions for static registry / UI catalogs.
    /// Agent-facing lists use [`Self::tools_for_isolation`] with the session mode.
    pub fn tools_static() -> Vec<MCPTool> {
        Self::tools_for_isolation(WorkspaceIsolationMode::Host)
    }

    pub fn tools_for_isolation(isolation: WorkspaceIsolationMode) -> Vec<MCPTool> {
        let profile = tools::CodeToolsProfile::from_isolation(isolation);
        let mut tools = Vec::new();
        tools.extend(tools::file_tools());
        tools.extend(tools::code_tools(profile));
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
        self.cleanup_shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        for handle in self.cleanup_tasks.drain(..) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests;
