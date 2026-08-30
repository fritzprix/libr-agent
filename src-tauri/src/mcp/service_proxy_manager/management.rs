use super::super::service_proxy::MCPServiceProxy;
use super::super::types::MCPResponse;
use super::proxy_config::resolve_startup_timeout_seconds;
use super::runtime_updates::{
    apply_discovery_timeout_finalize, emit_runtime_state, replace_runtime_state_store,
    update_runtime_state_store, RuntimeStateUpdateResult,
};
use super::MCPServiceProxyManager;
use crate::agent::runtime_state::SessionRuntimeState;
use crate::mcp::builtin::service_id::BuiltinServiceId;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyReadinessState {
    MissingProxy,
    Ready,
    AwaitSignal,
}

/// Per-session readiness watch + optional app handle for timeout finalize emits.
#[derive(Clone)]
pub struct ProxyReadinessEntry {
    pub ready_tx: Arc<tokio::sync::watch::Sender<bool>>,
    pub app_handle: Option<AppHandle>,
}

enum WaitOutcome {
    Ready,
    WatchError(String),
    TimedOut,
}

pub fn decide_proxy_readiness_state(
    proxy_exists: bool,
    has_readiness_signal: bool,
) -> ProxyReadinessState {
    if !proxy_exists {
        ProxyReadinessState::MissingProxy
    } else if has_readiness_signal {
        ProxyReadinessState::AwaitSignal
    } else {
        ProxyReadinessState::Ready
    }
}

pub(super) async fn shutdown_stdio_manager_with_timeout(
    stdio_mgr: super::super::session_isolation::SessionMCPManager,
    session_id: &str,
    context: &str,
) {
    match tokio::time::timeout(std::time::Duration::from_secs(3), stdio_mgr.shutdown_all()).await {
        Ok(_) => {
            log::debug!(
                "Successfully shut down stdio processes for session: {} ({})",
                session_id,
                context
            );
        }
        Err(_) => {
            log::warn!(
                "Timeout waiting for stdio processes shutdown in {} for session: {}; continuing",
                context,
                session_id
            );
        }
    }
}

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
    pub async fn get_proxy(&self, session_id: &str) -> Option<Arc<MCPServiceProxy>> {
        self.proxies.read().await.get(session_id).cloned()
    }

    /// Cancel resources owned by a session without destroying its proxy.
    ///
    /// Soft workflow cancellation keeps the proxy alive so the session can be
    /// resumed, therefore resource cleanup must be routed through the existing
    /// session-bound builtin server instances.
    pub async fn kill_session_processes(&self, session_id: &str) -> Result<usize, String> {
        let Some(proxy) = self.get_proxy(session_id).await else {
            log::debug!(
                "No MCP proxy found while cancelling resources for session {}",
                session_id
            );
            return Ok(0);
        };

        proxy.kill_session_processes().await
    }

    /// Mark that a user cancel stopped a foreground process. The next tool
    /// result consumes this marker so the workflow can continue normally.
    pub async fn mark_process_cancel_pending(&self, session_id: &str) {
        self.process_cancel_pending
            .lock()
            .await
            .insert(session_id.to_string());
    }

    /// Consume the foreground-process cancellation marker for a session.
    pub async fn take_process_cancel_pending(&self, session_id: &str) -> bool {
        self.process_cancel_pending.lock().await.remove(session_id)
    }

    /// Clear any stale process-only cancellation marker before a new workflow.
    pub async fn clear_process_cancel_pending(&self, session_id: &str) {
        self.process_cancel_pending.lock().await.remove(session_id);
        self.clear_process_cancel_retry_state(session_id).await;
    }

    /// Clear the process-cancellation retry budget after a non-cancelled tool
    /// execution or when a session starts a new workflow.
    pub async fn clear_process_cancel_retry_state(&self, session_id: &str) {
        self.process_cancel_retry_states
            .lock()
            .await
            .remove(session_id);
        self.process_cancel_retry_counts
            .lock()
            .await
            .remove(session_id);
    }

    /// Remember which exact tool call was interrupted by process cancellation
    /// without retaining potentially sensitive raw arguments.
    pub async fn record_process_cancelled_tool(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &str,
    ) {
        self.process_cancel_retry_states.lock().await.insert(
            session_id.to_string(),
            super::ProcessCancelRetryState {
                tool_name: tool_name.to_string(),
                arguments_digest: super::process_cancel_arguments_digest(arguments),
            },
        );
    }

    /// Consume the session's single retry for a previously cancelled tool.
    ///
    /// The tool name and argument fingerprint are retained for diagnostics, but
    /// the session-wide counter also blocks a retry that changes arguments to
    /// bypass the exact-match check.
    pub async fn process_cancel_retry_exhausted(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &str,
    ) -> bool {
        let exact_match = {
            let states = self.process_cancel_retry_states.lock().await;
            let Some(state) = states.get(session_id) else {
                return false;
            };
            state.tool_name == tool_name
                && state.arguments_digest == super::process_cancel_arguments_digest(arguments)
        };

        let mut retry_counts = self.process_cancel_retry_counts.lock().await;
        let retry_count = retry_counts.entry(session_id.to_string()).or_default();
        if *retry_count >= super::MAX_PROCESS_CANCEL_RETRIES {
            return true;
        }

        *retry_count += 1;
        if !exact_match {
            log::warn!(
                "Process-cancel retry changed tool or arguments for session {}; consuming session retry budget",
                session_id
            );
        }
        false
    }

    /// Clear retry state when a tool execution was not stopped by process cancel.
    pub async fn clear_process_cancel_retry_after_tool(
        &self,
        session_id: &str,
        process_was_cancelled: bool,
    ) {
        if !process_was_cancelled {
            self.clear_process_cancel_retry_state(session_id).await;
        }
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
        self.process_cancel_pending.lock().await.remove(session_id);
        self.process_cancel_retry_states
            .lock()
            .await
            .remove(session_id);
        self.process_cancel_retry_counts
            .lock()
            .await
            .remove(session_id);

        // 3. Shutdown stdio processes
        if let Some(stdio_mgr) = self.session_stdio_managers.write().await.remove(session_id) {
            shutdown_stdio_manager_with_timeout(stdio_mgr, session_id, "destroy_proxy").await;
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

    /// Wait for background tool loading to complete for a session.
    ///
    /// Sessions backed only by builtin tools are immediately ready once the proxy exists
    /// (no entry in the map). Sessions with external stdio/HTTP servers signal readiness
    /// via a `watch::channel`.
    ///
    /// # Arguments
    /// * `session_id` - The session to wait for
    /// * `timeout_secs` - Maximum seconds to wait before returning an error
    ///
    /// # Returns
    /// * `Ok(())` - Tools are loaded (or no external servers)
    /// * `Err(String)` - Timeout or channel error
    pub async fn wait_until_proxy_ready(
        &self,
        session_id: &str,
        timeout_secs: u64,
    ) -> Result<(), String> {
        let readiness_signal = {
            let map = self.proxy_readiness.read().await;
            map.get(session_id).cloned()
        };

        match decide_proxy_readiness_state(
            self.get_proxy(session_id).await.is_some(),
            readiness_signal.is_some(),
        ) {
            ProxyReadinessState::MissingProxy => {
                return Err(format!("No MCP proxy exists for session: {}", session_id));
            }
            ProxyReadinessState::Ready => {
                return Ok(());
            }
            ProxyReadinessState::AwaitSignal => {}
        }

        let mut rx = readiness_signal
            .expect("readiness signal must exist when state requires waiting")
            .ready_tx
            .subscribe();

        if *rx.borrow() {
            return Ok(()); // Already signaled true
        }

        // Convert away from `watch::Ref` before any further `.await` — that guard
        // is !Send and must not be held across awaits inside tokio::spawn tasks.
        let wait_outcome = match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            rx.wait_for(|v| *v),
        )
        .await
        {
            Ok(Ok(ready)) => {
                drop(ready);
                WaitOutcome::Ready
            }
            Ok(Err(e)) => WaitOutcome::WatchError(e.to_string()),
            Err(_) => WaitOutcome::TimedOut,
        };

        match wait_outcome {
            WaitOutcome::Ready => Ok(()),
            WaitOutcome::WatchError(e) => {
                // Discovery may still be waiting on a slow/failed stdio server while HTTP
                // (and builtin) tools are already usable. Finalize Session Ready (TimedOut
                // pending servers) instead of only clearing the wait map.
                if self.get_proxy(session_id).await.is_some() {
                    log::warn!(
                        "Proxy readiness watch failed for session {}: {}; finalizing with currently available tools",
                        session_id,
                        e
                    );
                    self.finalize_discovery_for_wait_timeout(
                        session_id,
                        &format!("Proxy readiness watch failed: {e}"),
                    )
                    .await;
                    return Ok(());
                }
                Err(format!("Proxy readiness watch error: {}", e))
            }
            WaitOutcome::TimedOut => {
                if self.get_proxy(session_id).await.is_some() {
                    log::warn!(
                        "Proxy tool loading timed out after {}s for session: {}; finalizing with currently available tools",
                        timeout_secs,
                        session_id
                    );
                    self.finalize_discovery_for_wait_timeout(
                        session_id,
                        &format!(
                            "Tool discovery timed out after {timeout_secs}s waiting for proxy readiness"
                        ),
                    )
                    .await;
                    return Ok(());
                }
                Err(format!(
                    "Proxy tool loading timed out after {}s for session: {}",
                    timeout_secs, session_id
                ))
            }
        }
    }

    /// Mark pending MCP servers TimedOut, flip Session Ready, wake waiters.
    /// Idempotent when initialization already left `pending`.
    pub(super) async fn finalize_discovery_for_wait_timeout(&self, session_id: &str, reason: &str) {
        let entry = {
            let map = self.proxy_readiness.read().await;
            map.get(session_id).cloned()
        };
        let app_handle = entry.as_ref().and_then(|e| e.app_handle.clone());

        let update_result = update_runtime_state_store(
            &self.runtime_states,
            session_id,
            app_handle.as_ref(),
            |state| {
                let applied = apply_discovery_timeout_finalize(state, reason);
                // Idempotent TimedOut skip must still open Session Ready for waiters
                // (e.g. empty external set already Success but ready was cleared).
                if !applied && state.proxy.exists {
                    state.proxy.ready = true;
                }
            },
        )
        .await;

        if let Some(entry) = entry {
            entry.ready_tx.send_replace(true);
        }
        self.proxy_readiness.write().await.remove(session_id);

        if update_result.changed {
            log::info!(
                "Finalized MCP discovery on wait timeout for session {} (emitted={})",
                session_id,
                update_result.emitted
            );
        }
    }

    /// Call a tool via the appropriate session proxy
    ///
    /// This is the primary entry point for tool execution from agent workflows.
    /// It routes both builtin and external tools through the session proxy so that
    /// tool availability, guided recovery, and session-scoped MCP dispatch share one path.
    ///
    /// # Arguments
    /// * `session_id` - The session making the tool call
    /// * `tool_name` - Name of the tool to invoke (e.g., "attachments__addAttachment" or "filesystem__read_file")
    /// * `args` - JSON arguments for the tool
    ///
    /// # Returns
    /// * `Ok(MCPResponse)` - Tool execution result
    /// * `Err(String)` - Error if proxy not found or tool execution fails
    ///
    /// # Example
    /// ```rust,ignore
    /// // Example needs DatabaseConnection and SessionManager initialized, so we use ignore
    /// let result = manager.call_tool(
    ///     "session-123",
    ///     "attachments__addAttachment",
    ///     serde_json::json!({"title": "My Note", "content": "Content"})
    /// ).await?;
    /// ```
    pub async fn call_tool(
        &self,
        session_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<MCPResponse, String> {
        let is_builtin = tool_name
            .split_once("__")
            .map(|(server, _)| BuiltinServiceId::from_alias(server).is_some())
            .unwrap_or(false);

        let proxy = match self.get_proxy(session_id).await {
            Some(proxy) if !is_builtin && proxy.is_builtin_only() => {
                log::debug!(
                    "Upgrading lazy builtin-only proxy for session {} before external tool '{}'",
                    session_id,
                    tool_name
                );
                let active_sessions = self.list_sessions().await;
                self.ensure_configured_proxy(session_id, None)
                    .await
                    .map_err(|error| {
                        log::error!(
                            "Failed to ensure configured proxy for session {} before external tool '{}': {}. Active sessions: {:?}",
                            session_id,
                            tool_name,
                            error,
                            active_sessions
                        );
                        error
                    })?
            }
            Some(proxy) => proxy,
            None => {
                let active_sessions = self.list_sessions().await;
                if is_builtin {
                    log::debug!(
                        "No proxy for session {}, attempting lazy builtin proxy init",
                        session_id
                    );
                    self.ensure_builtin_proxy(session_id).await.map_err(|error| {
                        log::error!(
                            "Failed to lazily init builtin proxy for session {}: {}. Active sessions: {:?}",
                            session_id,
                            error,
                            active_sessions
                        );
                        format!("Session context not found or expired (ID: {})", session_id)
                    })?
                } else {
                    log::debug!(
                        "Ensuring config-aware proxy for session {} before external tool '{}'",
                        session_id,
                        tool_name
                    );
                    self.ensure_configured_proxy(session_id, None)
                        .await
                        .map_err(|error| {
                            log::error!(
                                "Failed to ensure configured proxy for session {} before external tool '{}': {}. Active sessions: {:?}",
                                session_id,
                                tool_name,
                                error,
                                active_sessions
                            );
                            error
                        })?
                }
            }
        };

        if !is_builtin {
            let timeout_secs = self.startup_timeout_secs().await;
            self.wait_until_proxy_ready(session_id, timeout_secs)
                .await
                .map_err(|e| format!("Proxy not ready for external tool '{}': {}", tool_name, e))?;
        }

        proxy.call_tool(tool_name, args).await
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

    /// Returns true if the session currently has at least one channel-capable external server
    /// that advertises remote permission relay support.
    pub async fn session_has_permission_relay_channels(&self, session_id: &str) -> bool {
        let stdio_manager = {
            let stdio_managers = self.session_stdio_managers.read().await;
            stdio_managers.get(session_id).cloned()
        };

        if let Some(manager) = stdio_manager {
            if manager
                .list_channel_metadata()
                .await
                .into_iter()
                .any(|channel| channel.supports_permission_relay)
            {
                return true;
            }
        }

        let http_manager = {
            let http_managers = self.session_http_managers.read().await;
            http_managers.get(session_id).cloned()
        };

        if let Some(manager) = http_manager {
            return manager
                .list_channel_metadata()
                .await
                .into_iter()
                .any(|channel| channel.supports_permission_relay);
        }

        false
    }

    /// Returns true if the session currently has a connected channel-capable server with the
    /// given server name.
    pub async fn session_has_channel_server(&self, session_id: &str, server_name: &str) -> bool {
        let stdio_manager = {
            let stdio_managers = self.session_stdio_managers.read().await;
            stdio_managers.get(session_id).cloned()
        };

        if let Some(manager) = stdio_manager {
            if manager
                .list_channel_metadata()
                .await
                .into_iter()
                .any(|channel| channel.server_name == server_name)
            {
                return true;
            }
        }

        let http_manager = {
            let http_managers = self.session_http_managers.read().await;
            http_managers.get(session_id).cloned()
        };

        if let Some(manager) = http_manager {
            return manager
                .list_channel_metadata()
                .await
                .into_iter()
                .any(|channel| channel.server_name == server_name);
        }

        false
    }

    pub async fn broadcast_channel_permission_request(
        &self,
        session_id: &str,
        request: crate::mcp::types::ChannelPermissionRequest,
    ) -> Result<(), String> {
        let stdio_manager = {
            let stdio_managers = self.session_stdio_managers.read().await;
            stdio_managers.get(session_id).cloned()
        };

        let Some(manager) = stdio_manager else {
            return Ok(());
        };

        manager
            .broadcast_permission_request(request)
            .await
            .map_err(|error| error.to_string())
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
