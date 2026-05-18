use super::super::service_proxy::MCPServiceProxy;
use super::super::types::MCPResponse;
use super::runtime_updates::{
    emit_runtime_state, replace_runtime_state_store, update_runtime_state_store,
    RuntimeStateUpdateResult,
};
use super::MCPServiceProxyManager;
use crate::agent::runtime_state::SessionRuntimeState;
use crate::mcp::builtin::service_id::BuiltinServiceId;
use std::sync::Arc;
use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyReadinessState {
    MissingProxy,
    Ready,
    AwaitSignal,
}

pub fn decide_proxy_readiness_state(
    proxy_exists: bool,
    has_readiness_signal: bool,
    runtime_ready: bool,
) -> ProxyReadinessState {
    if !proxy_exists {
        ProxyReadinessState::MissingProxy
    } else if !has_readiness_signal || runtime_ready {
        // A runtime-ready proxy must not block on a stale readiness entry left behind
        // by an earlier background discovery cycle.
        ProxyReadinessState::Ready
    } else {
        ProxyReadinessState::AwaitSignal
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

    pub(super) async fn set_runtime_state(
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

    pub(super) async fn update_runtime_state<F>(
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

    /// Destroy a proxy and cleanup its resources
    ///
    /// This should be called when an agent session terminates to free resources.
    /// Builtin tool instances are automatically dropped when the proxy is removed.
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    pub async fn destroy_proxy(&self, session_id: &str) {
        // 1. Remove builtin proxy
        let proxy_removed = self.proxies.write().await.remove(session_id).is_some();

        // 2. Cleanup readiness signal (drops Sender, waking any waiters with RecvError)
        self.proxy_readiness.write().await.remove(session_id);
        self.runtime_states.write().await.remove(session_id);

        // 3. Shutdown stdio processes
        if let Some(stdio_mgr) = self.session_stdio_managers.write().await.remove(session_id) {
            tokio::spawn(async move {
                stdio_mgr.shutdown_all().await;
            });
        }

        // 4. Remove HTTP session manager (HTTP connections are shared, just remove the manager)
        self.session_http_managers.write().await.remove(session_id);

        // 5. Remove per-session creation guard (allows future re-creation of same session_id)
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
        let runtime_state = self.get_runtime_state(session_id).await;
        let readiness_signal = {
            let map = self.proxy_readiness.read().await;
            map.get(session_id).cloned()
        };

        match decide_proxy_readiness_state(
            self.get_proxy(session_id).await.is_some(),
            readiness_signal.is_some(),
            runtime_state.proxy.ready,
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
            .subscribe();

        if *rx.borrow() {
            return Ok(()); // Already signaled true
        }

        tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            rx.wait_for(|v| *v),
        )
        .await
        .map_err(|_| {
            format!(
                "Proxy tool loading timed out after {}s for session: {}",
                timeout_secs, session_id
            )
        })?
        .map_err(|e| format!("Proxy readiness watch error: {}", e))?;

        Ok(())
    }

    /// Call a tool via the appropriate session proxy
    ///
    /// This is the primary entry point for tool execution from agent workflows.
    /// It implements dual routing:
    /// - Builtin tools -> session proxy
    /// - External stdio tools -> session-specific stdio manager
    /// - External HTTP tools -> shared HTTP manager
    ///
    /// # Arguments
    /// * `session_id` - The session making the tool call
    /// * `tool_name` - Name of the tool to invoke (e.g., "attachments__add" or "filesystem__read_file")
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
    ///     "attachments__add",
    ///     serde_json::json!({"title": "My Note", "content": "Content"})
    /// ).await?;
    /// ```
    pub async fn call_tool(
        &self,
        session_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<MCPResponse, String> {
        // Builtin tools route through proxy (identified by known service alias prefix)
        let is_builtin = tool_name
            .split_once("__")
            .map(|(server, _)| BuiltinServiceId::from_alias(server).is_some())
            .unwrap_or(false);
        if is_builtin {
            let proxy = match self.get_proxy(session_id).await {
                Some(proxy) => proxy,
                None => {
                    // No proxy exists yet — lazily initialise a builtin-only proxy so that
                    // idle sessions (exist in DB but have not run a workflow in this app
                    // session) can still serve builtin tool calls such as content-store
                    // listing from AgentResourceAttachmentContext.
                    log::debug!(
                        "No proxy for session {}, attempting lazy builtin proxy init",
                        session_id
                    );
                    match self.ensure_builtin_proxy(session_id).await {
                        Ok(proxy) => proxy,
                        Err(e) => {
                            let active_sessions = self.list_sessions().await;
                            log::error!(
                                "Failed to lazily init proxy for session {}: {}. Active sessions: {:?}",
                                session_id,
                                e,
                                active_sessions
                            );
                            return Err(format!(
                                "Session context not found or expired (ID: {})",
                                session_id
                            ));
                        }
                    }
                }
            };
            return proxy.call_tool(tool_name, args).await;
        }

        // External tools: parse server__tool format
        let (server_name, real_tool_name) = tool_name
            .split_once("__")
            .ok_or_else(|| format!("Invalid tool name format: {}", tool_name))?;

        // Check if server exists in session-specific stdio manager first (primary check)
        let stdio_managers = self.session_stdio_managers.read().await;
        let has_stdio = stdio_managers
            .get(session_id)
            .map(|mgr| mgr.has_server(server_name))
            .unwrap_or(false);

        if has_stdio {
            // Route to session-specific stdio manager
            let manager = stdio_managers
                .get(session_id)
                .ok_or_else(|| format!("No stdio manager for session: {}", session_id))?;

            return manager
                .call_tool(server_name, real_tool_name, args)
                .await
                .map_err(|e| format!("{}", e));
        }
        drop(stdio_managers);

        // Otherwise, route to session-specific HTTP manager
        let http_managers = self.session_http_managers.read().await;
        let manager = http_managers
            .get(session_id)
            .ok_or_else(|| format!("No HTTP manager for session: {}", session_id))?;

        manager
            .call_tool(server_name, real_tool_name, args)
            .await
            .map_err(|e| format!("{}", e))
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
}
