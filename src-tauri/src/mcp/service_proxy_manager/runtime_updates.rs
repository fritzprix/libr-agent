use crate::agent::events::AgentEvent;
use crate::agent::runtime_state::{
    SessionRuntimeInitResult, SessionRuntimePhase, SessionRuntimeProxyMode,
    SessionRuntimeServerState, SessionRuntimeServerStatus, SessionRuntimeState,
    SessionRuntimeTransport,
};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeStateUpdateResult {
    pub(crate) runtime_state: SessionRuntimeState,
    pub(crate) changed: bool,
    pub(crate) emitted: bool,
}

pub(super) fn emit_runtime_state(
    session_id: &str,
    runtime_state: &SessionRuntimeState,
    app_handle: Option<&AppHandle>,
) -> bool {
    if let Some(app) = app_handle {
        let event = AgentEvent::SessionRuntimeStateUpdated {
            session_id: session_id.to_string(),
            runtime_state: runtime_state.clone(),
        };
        if let Err(error) = crate::agent::tauri_events::emit_agent_event(app, event) {
            log::warn!(
                "Failed to emit runtime state update for session {}: {}",
                session_id,
                error
            );
        }
        true
    } else {
        false
    }
}

pub(super) async fn mutate_runtime_state_store<F>(
    runtime_states: &Arc<RwLock<HashMap<String, SessionRuntimeState>>>,
    session_id: &str,
    update: F,
) -> RuntimeStateUpdateResult
where
    F: FnOnce(&mut SessionRuntimeState),
{
    {
        let mut states = runtime_states.write().await;
        let state = states
            .entry(session_id.to_string())
            .or_insert_with(SessionRuntimeState::default);
        let previous_state = state.clone();
        update(state);

        let changed = *state != previous_state;
        if changed {
            state.sequence = previous_state.sequence.saturating_add(1);
        }
        let runtime_state = if changed {
            state.clone()
        } else {
            previous_state
        };

        RuntimeStateUpdateResult {
            runtime_state,
            changed,
            emitted: false,
        }
    }
}

pub(super) async fn replace_runtime_state_store(
    runtime_states: &Arc<RwLock<HashMap<String, SessionRuntimeState>>>,
    session_id: &str,
    runtime_state: SessionRuntimeState,
) -> RuntimeStateUpdateResult {
    let mut states = runtime_states.write().await;
    let previous_sequence = states.get(session_id).map_or(0, |state| state.sequence);
    let mut normalized_runtime_state = runtime_state;
    normalized_runtime_state.sequence = previous_sequence;
    let changed = states.get(session_id) != Some(&normalized_runtime_state);

    if changed {
        normalized_runtime_state.sequence = previous_sequence.saturating_add(1);
        states.insert(session_id.to_string(), normalized_runtime_state.clone());

        return RuntimeStateUpdateResult {
            runtime_state: normalized_runtime_state,
            changed,
            emitted: false,
        };
    }

    RuntimeStateUpdateResult {
        runtime_state: normalized_runtime_state,
        changed,
        emitted: false,
    }
}

pub(super) async fn update_runtime_state_store<F>(
    runtime_states: &Arc<RwLock<HashMap<String, SessionRuntimeState>>>,
    session_id: &str,
    app_handle: Option<&AppHandle>,
    update: F,
) -> RuntimeStateUpdateResult
where
    F: FnOnce(&mut SessionRuntimeState),
{
    let mut result = mutate_runtime_state_store(runtime_states, session_id, update).await;
    if result.changed {
        result.emitted = emit_runtime_state(session_id, &result.runtime_state, app_handle);
    }
    result
}

pub(super) fn build_bootstrap_runtime_state(
    has_external_servers: bool,
    runtime_servers: Vec<SessionRuntimeServerState>,
) -> SessionRuntimeState {
    if has_external_servers {
        SessionRuntimeState::configured_initializing(runtime_servers)
    } else {
        SessionRuntimeState {
            sequence: 0,
            phase: SessionRuntimePhase::Initializing,
            proxy: crate::agent::runtime_state::SessionRuntimeProxyState {
                exists: false,
                mode: SessionRuntimeProxyMode::BuiltinOnly,
                ready: false,
            },
            initialization: crate::agent::runtime_state::SessionRuntimeInitializationState {
                current_step: Some("Initializing session environment".to_string()),
                result: SessionRuntimeInitResult::Pending,
                error: None,
                docker: None,
            },
            servers: Vec::new(),
        }
    }
}

pub(super) fn apply_config_load_failed(
    state: &mut SessionRuntimeState,
    has_external_servers: bool,
    runtime_servers: Vec<SessionRuntimeServerState>,
    error: String,
) {
    state.phase = SessionRuntimePhase::Failed;
    state.proxy.exists = false;
    state.proxy.mode = if has_external_servers {
        SessionRuntimeProxyMode::Configured
    } else {
        SessionRuntimeProxyMode::BuiltinOnly
    };
    state.proxy.ready = false;
    state.initialization.current_step = Some("Loading tool configurations".to_string());
    state.initialization.result = SessionRuntimeInitResult::Failed;
    state.initialization.error = Some(error);
    state.servers = runtime_servers;
}

pub(super) fn apply_loading_tool_config(state: &mut SessionRuntimeState) {
    state.set_current_step("Loading tool configurations");
}

pub(super) fn apply_http_connecting(state: &mut SessionRuntimeState, server_names: &[String]) {
    for server_name in server_names {
        state.upsert_server(
            server_name,
            SessionRuntimeTransport::Http,
            SessionRuntimeServerStatus::Connecting,
            0,
            None,
        );
    }
    refresh_discovery_step(state);
}

pub(super) fn apply_proxy_created(state: &mut SessionRuntimeState, has_external_servers: bool) {
    state.set_proxy_exists(true);
    state.proxy.mode = if has_external_servers {
        SessionRuntimeProxyMode::Configured
    } else {
        SessionRuntimeProxyMode::BuiltinOnly
    };
}

fn is_terminal_server_status(status: &SessionRuntimeServerStatus) -> bool {
    SessionRuntimeState::is_terminal_server_status(status)
}

/// Rebuild `current_step` from live server statuses so parallel stdio/HTTP
/// discovery does not leave a misleading transport-only label in the UI.
pub(super) fn refresh_discovery_step(state: &mut SessionRuntimeState) {
    if state.servers.is_empty() {
        return;
    }

    let total = state.servers.len();
    let done = state
        .servers
        .iter()
        .filter(|server| is_terminal_server_status(&server.status))
        .count();
    let pending: Vec<&str> = state
        .servers
        .iter()
        .filter(|server| !is_terminal_server_status(&server.status))
        .map(|server| server.name.as_str())
        .collect();

    if pending.is_empty() {
        return;
    }

    state.set_current_step(format!(
        "Loading MCP: {} ({}/{})",
        pending.join(", "),
        done,
        total
    ));
}

fn apply_completion_step(state: &mut SessionRuntimeState) {
    if state.servers.is_empty() {
        state.set_current_step("Session initialization complete");
        return;
    }

    let ready_names: Vec<&str> = state
        .servers
        .iter()
        .filter(|server| server.status == SessionRuntimeServerStatus::Ready)
        .map(|server| server.name.as_str())
        .collect();
    let unsuccessful_names: Vec<&str> = state
        .servers
        .iter()
        .filter(|server| SessionRuntimeState::is_unsuccessful_terminal_status(&server.status))
        .map(|server| server.name.as_str())
        .collect();

    match state.initialization.result {
        SessionRuntimeInitResult::Success => {
            state.set_current_step(format!("MCP ready: {}", ready_names.join(", ")));
        }
        SessionRuntimeInitResult::Partial => {
            state.set_current_step(format!(
                "MCP partial: {} failed/timed out ({}/{} ready)",
                unsuccessful_names.join(", "),
                ready_names.len(),
                state.servers.len()
            ));
        }
        SessionRuntimeInitResult::Failed => {
            state.set_current_step(format!("MCP failed: {}", unsuccessful_names.join(", ")));
        }
        SessionRuntimeInitResult::Pending => {
            state.set_current_step("Session initialization complete");
        }
    }
}

pub(super) fn apply_server_discovering(
    state: &mut SessionRuntimeState,
    server_name: &str,
    transport: SessionRuntimeTransport,
) {
    state.upsert_server(
        server_name,
        transport,
        SessionRuntimeServerStatus::DiscoveringTools,
        0,
        None,
    );
    refresh_discovery_step(state);
}

pub(super) fn apply_server_connecting(
    state: &mut SessionRuntimeState,
    server_name: &str,
    transport: SessionRuntimeTransport,
) {
    state.upsert_server(
        server_name,
        transport,
        SessionRuntimeServerStatus::Connecting,
        0,
        None,
    );
    refresh_discovery_step(state);
}

pub(super) fn apply_server_ready(
    state: &mut SessionRuntimeState,
    server_name: &str,
    transport: SessionRuntimeTransport,
    tool_count: usize,
) {
    // May overwrite TimedOut after discovery deadline; summary is recomputed
    // when finish_background_discovery / apply_initialization_complete runs.
    state.upsert_server(
        server_name,
        transport,
        SessionRuntimeServerStatus::Ready,
        tool_count,
        None,
    );
    refresh_discovery_step(state);
}

pub(super) fn apply_server_failed(
    state: &mut SessionRuntimeState,
    server_name: &str,
    transport: SessionRuntimeTransport,
    error: String,
) {
    state.upsert_server(
        server_name,
        transport,
        SessionRuntimeServerStatus::Failed,
        0,
        Some(error),
    );
    refresh_discovery_step(state);
}

pub(super) fn apply_initialization_complete(state: &mut SessionRuntimeState) {
    state.recompute_summary();
    apply_completion_step(state);
}

/// Finalize Session Ready after discovery deadline / soft wait timeout.
/// Marks non-terminal servers as `TimedOut`, recomputes summary, updates step.
/// Returns false when initialization was already finalized (idempotent).
pub(super) fn apply_discovery_timeout_finalize(
    state: &mut SessionRuntimeState,
    reason: &str,
) -> bool {
    if !state.finalize_discovery_timeout(reason) {
        return false;
    }
    apply_completion_step(state);
    true
}
