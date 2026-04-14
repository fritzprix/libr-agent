use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

use super::service_proxy::MCPServiceProxy;
use super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::session_isolation_config::SessionIsolationConfig;
use crate::session::SessionManager;

mod caching;
mod cleanup;
mod creation;
mod management;
#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests;

pub use caching::{persist_tool_cache_for_server, spawn_tool_cache_update};
pub use creation::{decide_existing_proxy_disposition, ExistingProxyDisposition};
pub use management::{decide_proxy_readiness_state, ProxyReadinessState};

/// Manages per-session MCP service proxies for isolated tool execution
///
/// Each agent session gets its own proxy instance with dedicated builtin server instances,
/// ensuring complete isolation of tool state and context across concurrent sessions.
///
/// # Session Isolation for External MCP Servers
///
/// - **Stdio servers**: Each session gets independent process instances via SessionMCPManager
/// - **HTTP servers**: Shared connections with session ID injection via HttpSessionManager
pub struct MCPServiceProxyManager {
    /// Map of session_id to session-specific proxy instances
    proxies: Arc<RwLock<HashMap<String, Arc<MCPServiceProxy>>>>,

    /// Session-specific stdio MCP server managers (lazy-spawned per session)
    session_stdio_managers: Arc<RwLock<HashMap<String, SessionMCPManager>>>,

    /// Session-specific HTTP MCP server managers (shared connections with session headers)
    session_http_managers: Arc<RwLock<HashMap<String, HttpSessionManager>>>,

    /// Shared SeaORM database connection for all sessions
    db: Arc<DatabaseConnection>,

    /// Shared SessionManager for workspace/attachments servers
    session_manager: Arc<SessionManager>,

    /// Background cleanup task handle
    cleanup_task: Arc<Mutex<Option<JoinHandle<()>>>>,

    /// Signal to stop the cleanup task
    cleanup_shutdown: Arc<AtomicBool>,

    /// Session isolation configuration
    config: SessionIsolationConfig,

    /// Readiness signal per session: true when background tool loading is complete.
    /// Sessions with no external servers are considered immediately ready (no entry).
    proxy_readiness: Arc<RwLock<HashMap<String, Arc<tokio::sync::watch::Sender<bool>>>>>,

    /// Per-session creation lock to prevent duplicate proxy creation under concurrent calls.
    /// Two concurrent `create_proxy` calls for the same session_id will serialize here;
    /// the second caller blocks until the first finishes and then hits the idempotency re-check.
    creation_guards: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl std::fmt::Debug for MCPServiceProxyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MCPServiceProxyManager")
            .field("proxies", &"<RwLock<HashMap>>")
            .field("session_stdio_managers", &"<RwLock<HashMap>>")
            .field("session_http_managers", &"<RwLock<HashMap>>")
            .field("db", &"<DatabaseConnection>")
            .field("session_manager", &self.session_manager)
            .field("cleanup_task", &"<Mutex<Option<JoinHandle>>>")
            .field(
                "cleanup_shutdown",
                &self.cleanup_shutdown.load(Ordering::Relaxed),
            )
            .field("config", &self.config)
            .field("proxy_readiness", &"<RwLock<HashMap>>")
            .finish()
    }
}

impl Drop for MCPServiceProxyManager {
    fn drop(&mut self) {
        // Signal cleanup task to stop
        self.cleanup_shutdown.store(true, Ordering::Relaxed);

        // Abort cleanup task
        if let Ok(mut task) = self.cleanup_task.try_lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
    }
}
