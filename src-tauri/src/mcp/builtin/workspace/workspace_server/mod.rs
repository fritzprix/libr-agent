mod lifecycle;
#[cfg(test)]
mod linked_worktree_tests;
mod path_validation;
mod retained;
mod service_context;
mod teamwork_paths;
mod user_skills_guard;

pub use retained::release_session_workspace_state;

pub(crate) use path_validation::{
    looks_like_confused_harness_path, path_operation_failure_guidance,
    path_validation_failure_guidance, with_harness_alias_hint,
};

use super::persistent_shell;
use super::terminal_manager;
use super::tools;
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

/// The file tool manager for `workspace_dir`; see `WorkspaceServer::get_file_tool_manager`.
pub(crate) fn file_tool_manager(
    workspace_dir: std::path::PathBuf,
    allow_linked_worktrees: bool,
) -> SecureFileManager {
    let git_dirs = crate::mcp::builtin::utils::git_metadata_dirs(&workspace_dir);
    let manager = if allow_linked_worktrees {
        let roots = crate::mcp::builtin::utils::linked_worktree_roots(&workspace_dir);
        SecureFileManager::new_scoped_with_extra_roots(workspace_dir, roots)
    } else {
        SecureFileManager::new_scoped_with_base_dir(workspace_dir)
    };
    manager.with_git_metadata_write_guard(git_dirs)
}

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
    pub(crate) shell_manager: Arc<persistent_shell::PersistentShellManager>,
    pub(crate) context_cache: ContextCache,
    /// Set once the session is force-terminated (explicit terminate/delete).
    /// Late tool dispatches must not create new processes. Shared by every server of
    /// the session and its shell manager, like the registry.
    pub(crate) terminated: Arc<std::sync::atomic::AtomicBool>,
    cleanup_shutdown: Arc<std::sync::atomic::AtomicBool>,
    cleanup_tasks: Vec<JoinHandle<()>>,
}

impl WorkspaceServer {
    /// Create a workspace server for host isolation (tests / legacy global registry).
    pub fn new(session_id: String, session_manager: Arc<SessionManager>) -> Self {
        Self::with_isolation(session_id, session_manager, WorkspaceIsolationMode::Host)
    }

    /// Create a workspace server bound to a session's workspace isolation mode, with its
    /// own process registry and shell manager.
    pub fn with_isolation(
        session_id: String,
        session_manager: Arc<SessionManager>,
        workspace_isolation: WorkspaceIsolationMode,
    ) -> Self {
        let state = retained::RetainedWorkspaceState::new();
        Self::with_state(session_id, session_manager, workspace_isolation, state)
    }

    /// The server a session's proxy uses. Its processes and persistent shell outlive
    /// the server, so a proxy created after reload or a config change still owns them.
    pub fn for_session(
        session_id: String,
        session_manager: Arc<SessionManager>,
        workspace_isolation: WorkspaceIsolationMode,
    ) -> Self {
        let state = retained::retained_state(&session_id);
        Self::with_state(session_id, session_manager, workspace_isolation, state)
    }

    fn with_state(
        session_id: String,
        session_manager: Arc<SessionManager>,
        workspace_isolation: WorkspaceIsolationMode,
        state: retained::RetainedWorkspaceState,
    ) -> Self {
        info!(
            "WorkspaceServer created for session: {} (isolation={})",
            session_id, workspace_isolation
        );
        let retained::RetainedWorkspaceState {
            process_registry,
            shell_manager,
            terminated,
        } = state;
        let cleanup_shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let process_cleanup_task =
            lifecycle::start_cleanup_task(process_registry.clone(), cleanup_shutdown.clone());

        Self {
            session_id,
            session_manager,
            workspace_isolation,
            isolation_manager: crate::session_isolation::SessionIsolationManager::new(),
            process_registry,
            shell_manager,
            context_cache: Arc::new(tokio::sync::RwLock::new(None)),
            terminated,
            cleanup_shutdown,
            cleanup_tasks: vec![process_cleanup_task],
        }
    }

    /// True once this instance's session has been force-terminated.
    pub(crate) fn is_session_terminated(&self) -> bool {
        self.terminated.load(std::sync::atomic::Ordering::SeqCst)
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

    /// The scoped manager used to validate file tool paths: the workspace root plus,
    /// when the `allowLinkedWorktrees` setting is on, the linked git worktrees of the
    /// workspace repository.
    pub(crate) async fn get_file_tool_manager(&self, session_id: &str) -> SecureFileManager {
        let allow = crate::models::system_settings::SystemSettings::load_global()
            .await
            .allow_linked_worktrees;
        file_tool_manager(self.get_workspace_dir(session_id), allow)
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
