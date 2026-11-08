use super::AgentSessionManager;

impl AgentSessionManager {
    /// Get available tools for a session based on agent configuration.
    ///
    /// Returns the filtered tool list that matches what the LLM will receive.
    /// Wait for background tool loading... stdio/HTTP servers are spawned asynchronously...
    /// partial tool list is far better than no list.
    /// Soft-wait uses the configured MCP discovery timeout (default 30s).
    pub async fn get_available_tools(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
        self.proxy_manager
            .ensure_configured_proxy(session_id, Some(self.app_handle.clone()))
            .await?;

        let timeout_secs = self.proxy_manager.startup_timeout_secs().await;
        if let Err(e) = self
            .proxy_manager
            .wait_until_proxy_ready(session_id, timeout_secs)
            .await
        {
            log::warn!(
                "Proxy readiness wait failed for session '{}': {}",
                session_id,
                e
            );
        }

        crate::agent::tools::collect_available_tools(session_id, &self.proxy_manager).await
    }

    pub async fn get_runtime_state(
        &self,
        session_id: &str,
    ) -> crate::agent::runtime_state::SessionRuntimeState {
        self.proxy_manager.get_runtime_state(session_id).await
    }

    /// Get available tools for a session based on its config
    pub async fn get_tools_for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
        self.get_available_tools(session_id).await
    }
}
