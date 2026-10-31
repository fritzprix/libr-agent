use super::super::super::service_proxy::MCPServiceProxy;
use super::super::super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::super::retry_policy::{FailureKind, RetryStateStore};
use crate::mcp::session_isolation::SessionMCPError;
use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tauri::AppHandle;

pub(super) type RuntimeStateStore =
    Arc<tokio::sync::RwLock<HashMap<String, crate::agent::runtime_state::SessionRuntimeState>>>;

#[derive(Clone)]
pub(super) struct DiscoveryContext {
    pub(super) session_id: String,
    pub(super) app_handle: Option<AppHandle>,
    pub(super) runtime_states: RuntimeStateStore,
    pub(super) server_name_to_id: Arc<HashMap<String, String>>,
    /// How long the session waits before it carries on without pending servers.
    pub(super) tool_discovery_timeout: Duration,
    /// Safety cap for one server; a server is only given up on after this (or on exit/error).
    pub(super) server_discovery_cap: Duration,
    pub(super) runtime_state_emits: Arc<AtomicU32>,
    pub(super) retry_states: RetryStateStore,
}

/// Why tool discovery of one server failed, and whether a later attempt can succeed.
#[derive(Debug)]
pub(super) struct DiscoveryFailure {
    pub(super) message: String,
    pub(super) kind: FailureKind,
}

pub(super) fn stdio_failure_kind(error: &SessionMCPError) -> FailureKind {
    match error {
        SessionMCPError::ContainerUnavailable(_) => FailureKind::Permanent,
        _ => FailureKind::Transient,
    }
}

pub(super) fn server_discovery_cap(session_wait: Duration) -> Duration {
    Duration::from_secs(crate::config::mcp_server_startup_cap_seconds(
        session_wait.as_secs(),
    ))
}

pub(super) struct PanicDiscoveryResult {
    pub(super) server_name: String,
    pub(super) panic_error: Option<String>,
}

pub(in super::super) struct BackgroundDiscoveryPlan {
    pub(in super::super) session_id: String,
    pub(in super::super) proxy: Arc<MCPServiceProxy>,
    pub(in super::super) stdio_manager: SessionMCPManager,
    pub(in super::super) http_manager: HttpSessionManager,
    pub(in super::super) stdio_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub(in super::super) http_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub(in super::super) server_name_to_id: HashMap<String, String>,
    pub(in super::super) tool_discovery_timeout: Duration,
    pub(in super::super) app_handle: Option<AppHandle>,
}

pub(super) fn describe_panic_payload(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_string();
    }
    "unknown panic payload".to_string()
}

pub(super) struct DiscoveryFinishStats {
    pub(super) stdio_server_count: usize,
    pub(super) http_server_count: usize,
    pub(super) total_start: Instant,
    pub(super) stdio_ms: u128,
    pub(super) http_ms: u128,
    pub(super) already_finalized_by_deadline: bool,
}

/// Parameters for retrying failed servers in the background.
pub(in super::super) struct RetryFailedServersParams {
    pub proxy: Arc<MCPServiceProxy>,
    pub stdio_manager: SessionMCPManager,
    pub http_manager: HttpSessionManager,
    pub stdio_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub http_configs: HashMap<String, crate::mcp::types::MCPServerConfig>,
    pub server_name_to_id: HashMap<String, String>,
    pub tool_discovery_timeout: Duration,
    pub app_handle: Option<AppHandle>,
}
