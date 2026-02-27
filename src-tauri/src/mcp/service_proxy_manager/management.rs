use super::super::service_proxy::MCPServiceProxy;
use super::super::types::MCPResponse;
use super::MCPServiceProxyManager;
use crate::mcp::builtin::service_id::is_builtin_tool_name;
use std::sync::Arc;

impl MCPServiceProxyManager {
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
    /// Sessions backed only by builtin tools are immediately ready (no entry in the map).
    /// Sessions with external stdio/HTTP servers signal readiness via a `watch::channel`.
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
        let rx = {
            let map = self.proxy_readiness.read().await;
            map.get(session_id).map(|tx| tx.subscribe())
        };

        let Some(mut rx) = rx else {
            // No entry = no external servers = already ready
            return Ok(());
        };

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
    /// * `tool_name` - Name of the tool to invoke (e.g., "builtin_attachments__addContent" or "filesystem__read_file")
    /// * `args` - JSON arguments for the tool
    ///
    /// # Returns
    /// * `Ok(MCPResponse)` - Tool execution result
    /// * `Err(String)` - Error if proxy not found or tool execution fails
    ///
    /// # Example
    /// ```rust,ignore
    /// let result = manager.call_tool(
    ///     "session-123",
    ///     "builtin_attachments__addContent",
    ///     json!({"title": "My Note", "content": "Content"})
    /// ).await?;
    /// ```
    pub async fn call_tool(
        &self,
        session_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<MCPResponse, String> {
        // Builtin tools route through proxy
        if is_builtin_tool_name(tool_name) {
            let proxy = match self.get_proxy(session_id).await {
                Some(proxy) => proxy,
                None => {
                    let active_sessions = self.list_sessions().await;
                    log::error!(
                        "No proxy found for session: {}. Active sessions: {:?}",
                        session_id,
                        active_sessions
                    );
                    return Err(format!(
                        "Session context not found or expired (ID: {})",
                        session_id
                    ));
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
}
