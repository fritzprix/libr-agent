//! Retry of failed external MCP servers: the manual single-server entry point
//! and the shared claimed-server retry engine for manual and automatic triggers.

use super::super::retry_policy::RetryTrigger;
use super::MCPServiceProxyManager;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tauri::AppHandle;

impl MCPServiceProxyManager {
    /// Manually retry a failed MCP server in an active session. A server that is not `Failed`
    /// (ready, or still starting in the background) is left alone.
    pub async fn retry_failed_server(
        &self,
        session_id: &str,
        server_name: &str,
        app_handle: Option<&AppHandle>,
    ) -> Result<(), String> {
        let state = self.get_runtime_state(session_id).await;
        let Some(server) = state.servers.iter().find(|s| s.name == server_name) else {
            return Err(format!(
                "Server '{}' not found in session '{}'",
                server_name, session_id
            ));
        };
        if server.status != crate::agent::runtime_state::SessionRuntimeServerStatus::Failed {
            return Ok(());
        }
        self.retry_failed_servers(
            session_id,
            Some(server_name),
            RetryTrigger::Manual,
            app_handle,
        )
        .await
        .map(|_| ())
    }

    /// Retries the session's `Failed` servers (only `only` when given) and returns how many
    /// were started. The `Failed -> Connecting` move happens under the runtime state lock before
    /// anything is spawned, so a second trigger for the same server (a double click, re-entering
    /// the session twice) finds it `Connecting` and starts nothing.
    ///
    /// An automatic retry skips servers that are still in their backoff delay, that reached
    /// the retry limit, or whose failure cannot change between attempts. A manual retry starts
    /// them all and resets their failure count.
    pub(in crate::mcp::service_proxy_manager) async fn retry_failed_servers(
        &self,
        session_id: &str,
        only: Option<&str>,
        trigger: RetryTrigger,
        app_handle: Option<&AppHandle>,
    ) -> Result<usize, String> {
        let proxy = self
            .proxies
            .read()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("No active proxy for session '{}'", session_id))?;
        let stdio_mgr = self
            .session_stdio_managers
            .read()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("No stdio manager for session '{}'", session_id))?;
        let http_mgr = self
            .session_http_managers
            .read()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("No http manager for session '{}'", session_id))?;
        let loaded = self
            .session_server_configs
            .read()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("No server configs cached for session '{}'", session_id))?;

        if let Some(name) = only {
            if !loaded.stdio_configs.contains_key(name) && !loaded.http_configs.contains_key(name) {
                return Err(format!(
                    "Configuration for server '{}' not found in session '{}'",
                    name, session_id
                ));
            }
        }

        let auto_allowed = match trigger {
            RetryTrigger::Manual => None,
            RetryTrigger::Automatic => {
                let failed: Vec<String> = self
                    .get_runtime_state(session_id)
                    .await
                    .servers
                    .into_iter()
                    .filter(|server| {
                        server.status
                            == crate::agent::runtime_state::SessionRuntimeServerStatus::Failed
                    })
                    .map(|server| server.name)
                    .collect();
                if failed.is_empty() {
                    return Ok(0);
                }
                let allowed = self
                    .server_retry_states
                    .auto_retry_allowed(session_id, &failed, Instant::now())
                    .await;
                if allowed.len() < failed.len() {
                    log::debug!(
                        "Automatic retry skips {} failed MCP server(s) in backoff or at the retry limit for session '{}'",
                        failed.len() - allowed.len(),
                        session_id
                    );
                }
                Some(allowed)
            }
        };

        let mut claimed = Vec::new();
        super::super::runtime_updates::update_runtime_state_store(
            &self.runtime_states,
            session_id,
            app_handle,
            |state| {
                claimed =
                    super::super::runtime_updates::claim_failed_servers_for_retry(state, |name| {
                        only.is_none_or(|only| only == name)
                            && auto_allowed
                                .as_ref()
                                .is_none_or(|allowed| allowed.iter().any(|allowed| allowed == name))
                            && (loaded.stdio_configs.contains_key(name)
                                || loaded.http_configs.contains_key(name))
                    });
            },
        )
        .await;
        if claimed.is_empty() {
            return Ok(0);
        }
        if trigger == RetryTrigger::Manual {
            for name in &claimed {
                self.server_retry_states
                    .clear_server(session_id, name)
                    .await;
            }
        }

        let retry_stdio: HashMap<_, _> = loaded
            .stdio_configs
            .iter()
            .filter(|(name, _)| claimed.contains(name))
            .map(|(name, config)| (name.clone(), config.clone()))
            .collect();
        let retry_http: HashMap<_, _> = loaded
            .http_configs
            .iter()
            .filter(|(name, _)| claimed.contains(name))
            .map(|(name, config)| (name.clone(), config.clone()))
            .collect();
        log::info!(
            "Retrying {} failed MCP server(s) for session '{}': {:?}",
            claimed.len(),
            session_id,
            claimed
        );

        let tool_discovery_timeout = Duration::from_secs(self.startup_timeout_secs().await);
        super::super::background_discovery::spawn_retry_failed_servers(
            self,
            session_id.to_string(),
            super::super::background_discovery::RetryFailedServersParams {
                proxy,
                stdio_manager: stdio_mgr,
                http_manager: http_mgr,
                stdio_configs: retry_stdio,
                http_configs: retry_http,
                server_name_to_id: loaded.server_name_to_id.clone(),
                secret_values: loaded.secret_values.clone(),
                tool_discovery_timeout,
                app_handle: app_handle.cloned(),
            },
        )
        .await;
        Ok(claimed.len())
    }
}
