//! Workspace state that belongs to a session rather than to one `WorkspaceServer`.
//!
//! Reload and config-driven recreation replace the session's proxy and its
//! `WorkspaceServer`, while `spawnProcess` children and the persistent shell keep
//! running. The process registry, shell manager and terminate flag therefore live
//! here, keyed by session, so the next server lists, reads, and stops the same
//! processes, and a terminate marked through any server fences them all. Only an
//! explicit terminate or delete releases them.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

use super::{persistent_shell, terminal_manager};

#[derive(Clone)]
pub(crate) struct RetainedWorkspaceState {
    pub(crate) process_registry: terminal_manager::ProcessRegistry,
    pub(crate) shell_manager: Arc<persistent_shell::PersistentShellManager>,
    /// The shell manager is bound to this same flag.
    pub(crate) terminated: Arc<AtomicBool>,
}

impl RetainedWorkspaceState {
    pub(crate) fn new() -> Self {
        let terminated = Arc::new(AtomicBool::new(false));
        Self {
            process_registry: terminal_manager::create_process_registry(),
            shell_manager: Arc::new(
                persistent_shell::PersistentShellManager::new()
                    .with_terminated_flag(terminated.clone()),
            ),
            terminated,
        }
    }
}

static RETAINED: Lazy<Mutex<HashMap<String, RetainedWorkspaceState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// The session's retained state, created on first use.
pub(crate) fn retained_state(session_id: &str) -> RetainedWorkspaceState {
    RETAINED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(session_id.to_string())
        .or_insert_with(RetainedWorkspaceState::new)
        .clone()
}

/// Drops the session's retained process registry, shell manager and terminate flag. Call on explicit
/// terminate or delete, after session-owned processes were stopped; a server created
/// afterwards starts empty.
pub fn release_session_workspace_state(session_id: &str) -> bool {
    RETAINED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(session_id)
        .is_some()
}
