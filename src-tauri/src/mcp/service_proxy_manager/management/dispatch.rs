//! Tool-call dispatch through the session proxy and channel-server queries
//! used by permission relay routing.

use super::MCPServiceProxyManager;
use crate::mcp::builtin::service_id::BuiltinServiceId;
use crate::mcp::types::MCPResponse;

impl MCPServiceProxyManager {
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
        // Force-stop fence: capture the generation at entry so a terminate
        // (and even a later clear) during proxy creation or readiness waits
        // still rejects this dispatch before it can spawn resources
        // (issue #2633 G2.1).
        let fence_generation = self.teardown_fence_generation(session_id).await;
        self.reject_if_teardown_fenced(session_id).await?;

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
                        // A force-stop fence refusal must reach the caller with
                        // its marker so it is recognized as a correct refusal,
                        // not a lost session context (issue #2633 G2.3).
                        if Self::is_teardown_fence_rejection(&error) {
                            error
                        } else {
                            format!("Session context not found or expired (ID: {})", session_id)
                        }
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

        // Re-check after resolution and readiness waits: the fence can be set
        // (or set and cleared again, bumping the generation) while this
        // dispatch is still awaiting, and a proxy that was created during the
        // fence must never be handed a tool call (issue #2633 G2.1).
        self.reject_if_teardown_fence_moved(session_id, fence_generation)
            .await?;

        proxy.call_tool(tool_name, args).await
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
