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

pub(super) fn emit_runtime_state(
    session_id: &str,
    runtime_state: &SessionRuntimeState,
    app_handle: Option<&AppHandle>,
) {
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
    }
}

pub(super) async fn mutate_runtime_state_store<F>(
    runtime_states: &Arc<RwLock<HashMap<String, SessionRuntimeState>>>,
    session_id: &str,
    update: F,
) -> SessionRuntimeState
where
    F: FnOnce(&mut SessionRuntimeState),
{
    let runtime_state = {
        let mut states = runtime_states.write().await;
        let state = states
            .entry(session_id.to_string())
            .or_insert_with(SessionRuntimeState::default);
        update(state);
        state.clone()
    };

    runtime_state
}

pub(super) async fn update_runtime_state_store<F>(
    runtime_states: &Arc<RwLock<HashMap<String, SessionRuntimeState>>>,
    session_id: &str,
    app_handle: Option<&AppHandle>,
    update: F,
) -> SessionRuntimeState
where
    F: FnOnce(&mut SessionRuntimeState),
{
    let runtime_state = mutate_runtime_state_store(runtime_states, session_id, update).await;
    emit_runtime_state(session_id, &runtime_state, app_handle);
    runtime_state
}

pub(super) fn build_bootstrap_runtime_state(
    has_external_servers: bool,
    runtime_servers: Vec<SessionRuntimeServerState>,
) -> SessionRuntimeState {
    if has_external_servers {
        SessionRuntimeState::configured_initializing(runtime_servers)
    } else {
        SessionRuntimeState {
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
    state.set_current_step("Connecting to HTTP tool servers");
    for server_name in server_names {
        state.upsert_server(
            server_name,
            SessionRuntimeTransport::Http,
            SessionRuntimeServerStatus::Connecting,
            0,
            None,
        );
    }
}

pub(super) fn apply_proxy_created(state: &mut SessionRuntimeState, has_external_servers: bool) {
    state.set_proxy_exists(true);
    state.proxy.mode = if has_external_servers {
        SessionRuntimeProxyMode::Configured
    } else {
        SessionRuntimeProxyMode::BuiltinOnly
    };
}

pub(super) fn apply_batch_step(state: &mut SessionRuntimeState, step: impl Into<String>) {
    state.set_current_step(step);
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
}

pub(super) fn apply_server_ready(
    state: &mut SessionRuntimeState,
    server_name: &str,
    transport: SessionRuntimeTransport,
    tool_count: usize,
) {
    state.upsert_server(
        server_name,
        transport,
        SessionRuntimeServerStatus::Ready,
        tool_count,
        None,
    );
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
}

pub(super) fn apply_initialization_complete(state: &mut SessionRuntimeState) {
    state.set_current_step("Session initialization complete");
    state.recompute_summary();
}
