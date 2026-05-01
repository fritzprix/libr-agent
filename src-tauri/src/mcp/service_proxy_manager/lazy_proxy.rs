use super::super::service_proxy::MCPServiceProxy;
use super::super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::super::session_isolation_config::SessionIsolationConfig;
use super::MCPServiceProxyManager;
use crate::agent::runtime_state::SessionRuntimeState;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

impl MCPServiceProxyManager {
    /// Ensure a session has a proxy that matches its persisted agent configuration.
    ///
    /// Unlike `ensure_builtin_proxy`, this path is config-aware: it loads the session's
    /// stored `agent_config`, derives both builtin and external MCP requirements, and then
    /// delegates to `create_proxy()`. That means an existing builtin-only lazy proxy will
    /// be recreated when the session configuration requires stdio/HTTP MCP servers.
    pub async fn ensure_configured_proxy(
        &self,
        session_id: &str,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<Arc<MCPServiceProxy>, String> {
        use crate::agent::tools::extract_builtin_tool_ids;
        use crate::repositories::session_repository::SessionRepository;

        let session_repo = crate::state::get_session_repository();
        let session = session_repo
            .get_session(session_id)
            .await
            .map_err(|e| format!("Failed to load session {}: {}", session_id, e))?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        let config_json = session
            .agent_config
            .ok_or_else(|| "Session has no config".to_string())?;
        let agent_config = crate::agent::AgentConfig::from_json(&config_json)?;
        let tool_ids = extract_builtin_tool_ids(&agent_config);

        self.create_proxy(
            session_id.to_string(),
            tool_ids,
            agent_config.mcp_server_ids,
            app_handle,
        )
        .await
    }

    /// Lazily initialise a builtin-only proxy for a session that has no active proxy.
    ///
    /// Called by [`crate::mcp::service_proxy_manager::MCPServiceProxyManager::call_tool`] when a builtin tool is requested for a session whose proxy
    /// (e.g., a session that exists in the DB but has not yet run
    /// a workflow in this app session). This prevents spurious "Session context not found"
    /// errors when UI components poll builtin tools (e.g., attachments listing) for
    /// idle sessions.
    ///
    /// Only builtins are wired up — no external stdio/HTTP servers are started.
    /// If the session's agent config cannot be loaded, falls back to
    /// `CORE_BUILTIN_SERVICE_ALIASES` (the full non-optional builtin set).
    pub async fn ensure_builtin_proxy(
        &self,
        session_id: &str,
    ) -> Result<Arc<MCPServiceProxy>, String> {
        if let Some(existing) = self.get_proxy(session_id).await {
            return Ok(existing);
        }

        let session_guard = {
            let mut guards = self.creation_guards.lock().await;
            guards
                .entry(session_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _lock = session_guard.lock().await;

        if let Some(existing) = self.get_proxy(session_id).await {
            return Ok(existing);
        }

        log::debug!(
            "Lazily initialising builtin-only proxy for idle session: {}",
            session_id
        );

        let tool_ids = self.resolve_tool_ids_for_session(session_id).await;
        let workspace_dir =
            crate::session::resolve_session_workspace_dir(&self.session_manager, session_id)
                .await?;
        let empty_stdio = SessionMCPManager::new(
            session_id.to_string(),
            HashMap::new(),
            SessionIsolationConfig::default(),
            workspace_dir,
        );
        let empty_http = HttpSessionManager::new(session_id.to_string(), HashMap::new());

        let proxy = MCPServiceProxy::builder(
            session_id.to_string(),
            self.db.clone(),
            self.session_manager.clone(),
            Arc::new(empty_http.clone()),
            Arc::new(empty_stdio.clone()),
        )
        .with_tool_ids(tool_ids.clone())
        .build()
        .await?;

        let proxy_arc = Arc::new(proxy);
        self.proxies
            .write()
            .await
            .insert(session_id.to_string(), proxy_arc.clone());
        self.session_stdio_managers
            .write()
            .await
            .insert(session_id.to_string(), empty_stdio);
        self.session_http_managers
            .write()
            .await
            .insert(session_id.to_string(), empty_http);
        self.set_runtime_state(session_id, SessionRuntimeState::builtin_ready(), None)
            .await;

        log::info!(
            "Lazily initialised builtin-only proxy for idle session {} with tools: {:?}",
            session_id,
            tool_ids
        );

        Ok(proxy_arc)
    }

    /// Resolve the builtin tool IDs for a session by reading its `agent_config` from the DB.
    ///
    /// Falls back to [`CORE_BUILTIN_SERVICE_ALIASES`] if the session or its config cannot
    /// be loaded.
    async fn resolve_tool_ids_for_session(&self, session_id: &str) -> Vec<String> {
        use crate::agent::tools::extract_builtin_tool_ids;
        use crate::mcp::builtin::service_id::CORE_BUILTIN_SERVICE_ALIASES;
        use crate::repositories::session_repository::SessionRepository;

        let repo = crate::state::get_session_repository();
        match repo.get_session(session_id).await {
            Ok(Some(session)) => {
                if let Some(config_str) = &session.agent_config {
                    if let Ok(agent_config) = crate::agent::AgentConfig::from_json(config_str) {
                        return extract_builtin_tool_ids(&agent_config);
                    }
                }
            }
            Ok(None) => {
                log::warn!(
                    "Session {} not found in DB during lazy proxy init; using core builtins",
                    session_id
                );
            }
            Err(error) => {
                log::warn!(
                    "DB error loading session {} for lazy proxy init: {}; using core builtins",
                    session_id,
                    error
                );
            }
        }

        CORE_BUILTIN_SERVICE_ALIASES
            .iter()
            .map(|service_id| service_id.to_string())
            .collect()
    }
}
