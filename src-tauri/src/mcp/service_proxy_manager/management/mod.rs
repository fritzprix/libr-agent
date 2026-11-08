//! `MCPServiceProxyManager` lifecycle and runtime-state surface.
//!
//! Split from a single 944-line `impl` block (issue #2648). Call sites keep
//! importing from `super::management::*` unchanged:
//! - [`readiness`] — proxy readiness state machine, wait, timeout finalize
//! - [`retry`] — failed-server retry engine (manual + automatic)
//! - [`teardown`] — force-stop fence lifecycle + process cancel markers
//! - [`dispatch`] — `call_tool` + channel-server queries
//!
//! This module holds the small runtime-state accessors, the per-session
//! creation guard, proxy destroy/count/list, and the test helpers.

mod dispatch;
mod readiness;
mod retry;
mod teardown;

pub use readiness::{decide_proxy_readiness_state, ProxyReadinessEntry, ProxyReadinessState};
pub(super) use teardown::shutdown_stdio_manager_with_timeout;

use super::proxy_config::resolve_startup_timeout_seconds;
use super::runtime_updates::{
    emit_runtime_state, replace_runtime_state_store, update_runtime_state_store,
    RuntimeStateUpdateResult,
};
use super::MCPServiceProxyManager;
use crate::agent::runtime_state::SessionRuntimeState;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;

impl MCPServiceProxyManager {
    pub async fn get_runtime_state(&self, session_id: &str) -> SessionRuntimeState {
        self.runtime_states
            .read()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Effective MCP discovery / soft-wait timeout (settings or default 30s).
    pub async fn startup_timeout_secs(&self) -> u64 {
        resolve_startup_timeout_seconds(&self.config).await
    }

    pub(crate) async fn set_runtime_state(
        &self,
        session_id: &str,
        runtime_state: SessionRuntimeState,
        app_handle: Option<&AppHandle>,
    ) -> RuntimeStateUpdateResult {
        let mut result =
            replace_runtime_state_store(&self.runtime_states, session_id, runtime_state).await;
        if result.changed {
            result.emitted = emit_runtime_state(session_id, &result.runtime_state, app_handle);
        }
        result
    }

    pub(crate) async fn update_runtime_state<F>(
        &self,
        session_id: &str,
        app_handle: Option<&AppHandle>,
        update: F,
    ) -> RuntimeStateUpdateResult
    where
        F: FnOnce(&mut SessionRuntimeState),
    {
        update_runtime_state_store(&self.runtime_states, session_id, app_handle, update).await
    }

    /// Get an existing proxy for a session
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    /// * `Some(Arc<MCPServiceProxy>)` - Existing proxy instance
    /// * `None` - No proxy found for this session
    pub async fn get_proxy(
        &self,
        session_id: &str,
    ) -> Option<Arc<super::super::service_proxy::MCPServiceProxy>> {
        self.proxies.read().await.get(session_id).cloned()
    }

    /// Per-session creation mutex used by create / ensure_builtin / destroy.
    ///
    /// The outer `creation_guards` map lock is held only long enough to insert or look up
    /// the per-session `Arc<Mutex<()>>`. Callers hold the returned mutex across publish
    /// (and, for create, across HTTP startup) for single-flight semantics.
    pub(super) async fn session_creation_guard(&self, session_id: &str) -> Arc<Mutex<()>> {
        let mut guards = self.creation_guards.lock().await;
        guards
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Destroy a proxy and cleanup its resources
    ///
    /// This should be called when an agent session terminates to free resources.
    /// Builtin tool instances are automatically dropped when the proxy is removed.
    /// Serialized with in-flight `create_proxy` / `ensure_builtin_proxy` via the
    /// per-session creation lock.
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    pub async fn destroy_proxy(&self, session_id: &str) {
        let session_guard = self.session_creation_guard(session_id).await;
        let _session_lock = session_guard.lock().await;

        // 1. Remove builtin proxy
        let proxy_removed = self.proxies.write().await.remove(session_id).is_some();

        // 2. Cleanup readiness signal (drops Sender, waking any waiters with RecvError)
        self.proxy_readiness.write().await.remove(session_id);
        self.runtime_states.write().await.remove(session_id);
        self.session_server_configs.write().await.remove(session_id);
        self.server_retry_states.clear_session(session_id).await;
        self.process_cancel_pending.lock().await.remove(session_id);

        // 3. Shutdown stdio processes
        if let Some(stdio_mgr) = self.session_stdio_managers.write().await.remove(session_id) {
            teardown::shutdown_stdio_manager_with_timeout(stdio_mgr, session_id, "destroy_proxy")
                .await;
        }

        // 4. Remove HTTP session manager (HTTP connections are shared, just remove the manager)
        self.session_http_managers.write().await.remove(session_id);

        // 5. Remove per-session creation guard while still holding `_session_lock`
        //    so waiters cannot observe a missing guard mid-teardown.
        self.creation_guards.lock().await.remove(session_id);

        if proxy_removed {
            log::info!("Destroyed all resources for session: {}", session_id);
        } else {
            log::warn!(
                "Attempted to destroy non-existent proxy for session: {}",
                session_id
            );
        }
    }

    /// Get the number of active proxies
    ///
    /// Useful for monitoring and debugging
    pub async fn proxy_count(&self) -> usize {
        self.proxies.read().await.len()
    }

    /// List all active session IDs
    ///
    /// Useful for monitoring and debugging
    pub async fn list_sessions(&self) -> Vec<String> {
        self.proxies.read().await.keys().cloned().collect()
    }
}

#[cfg(test)]
impl MCPServiceProxyManager {
    pub(crate) async fn mark_runtime_proxy_not_ready_for_test(&self, session_id: &str) {
        self.force_runtime_proxy_not_ready_for_test(session_id)
            .await;
    }
}

impl MCPServiceProxyManager {
    /// Integration-test helper: clear `proxy.ready` without destroying the proxy Arc.
    ///
    /// Used to reproduce the Hydrating stuck case where a proxy exists but the
    /// runtime snapshot still reports not-ready (e.g. lost emit after lazy init).
    pub async fn force_runtime_proxy_not_ready_for_test(&self, session_id: &str) {
        let mut state = self.get_runtime_state(session_id).await;
        state.proxy.ready = false;
        self.set_runtime_state(session_id, state, None).await;
    }
}
