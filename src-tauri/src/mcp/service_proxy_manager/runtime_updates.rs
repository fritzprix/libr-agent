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

    let timed_out_names: Vec<&str> = state
        .servers
        .iter()
        .filter(|server| server.status == SessionRuntimeServerStatus::TimedOut)
        .map(|server| server.name.as_str())
        .collect();

    match state.initialization.result {
        SessionRuntimeInitResult::Success => {
            if timed_out_names.is_empty() {
                state.set_current_step(format!("MCP ready: {}", ready_names.join(", ")));
            } else if ready_names.is_empty() {
                state.set_current_step(format!(
                    "MCP still starting: {}",
                    timed_out_names.join(", ")
                ));
            } else {
                state.set_current_step(format!(
                    "MCP ready: {} (still starting: {})",
                    ready_names.join(", "),
                    timed_out_names.join(", ")
                ));
            }
        }
        SessionRuntimeInitResult::Partial => {
            state.set_current_step(format!(
                "MCP partial: {} failed ({}/{} ready)",
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

/// The last failure reason of a server. A retry keeps it until the server is ready or fails
/// again, so the reason stays visible while the server restarts.
fn last_server_error(state: &SessionRuntimeState, server_name: &str) -> Option<String> {
    state
        .servers
        .iter()
        .find(|server| server.name == server_name)
        .and_then(|server| server.error.clone())
}

pub(super) fn apply_server_discovering(
    state: &mut SessionRuntimeState,
    server_name: &str,
    transport: SessionRuntimeTransport,
) {
    let error = last_server_error(state, server_name);
    state.upsert_server(
        server_name,
        transport,
        SessionRuntimeServerStatus::DiscoveringTools,
        0,
        error,
    );
    refresh_discovery_step(state);
}

pub(super) fn apply_server_connecting(
    state: &mut SessionRuntimeState,
    server_name: &str,
    transport: SessionRuntimeTransport,
) {
    let error = last_server_error(state, server_name);
    state.upsert_server(
        server_name,
        transport,
        SessionRuntimeServerStatus::Connecting,
        0,
        error,
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

/// The discovery task ended without finishing its servers (it panicked or was aborted). No task
/// is starting them any more, so every server that is not `Ready` or `Failed` is a failure, not
/// a server that is "still starting"; that makes it a retry target. Recomputes the summary.
pub(super) fn apply_discovery_task_failed(state: &mut SessionRuntimeState, reason: &str) {
    for server in &mut state.servers {
        if !matches!(
            server.status,
            SessionRuntimeServerStatus::Ready | SessionRuntimeServerStatus::Failed
        ) {
            server.status = SessionRuntimeServerStatus::Failed;
            server.error = Some(reason.to_string());
        }
    }
    state.recompute_summary();
    apply_completion_step(state);
}

/// Starts a retry: moves each `Failed` server accepted by `wanted` to `Connecting` in one step
/// and returns their names. A server that is already `Connecting`, `DiscoveringTools`,
/// `TimedOut` (still starting in the background) or `Ready` is not returned, so a second trigger
/// for the same server finds nothing to retry. This is the only way a retry is started.
/// The last failure reason is kept.
pub(super) fn claim_failed_servers_for_retry(
    state: &mut SessionRuntimeState,
    wanted: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut claimed = Vec::new();
    for server in &mut state.servers {
        if server.status == SessionRuntimeServerStatus::Failed && wanted(&server.name) {
            server.status = SessionRuntimeServerStatus::Connecting;
            server.tool_count = 0;
            claimed.push(server.name.clone());
        }
    }
    if !claimed.is_empty() {
        refresh_discovery_step(state);
    }
    claimed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured_state() -> SessionRuntimeState {
        let mut state = SessionRuntimeState::default();
        state.proxy.exists = true;
        state.proxy.mode = SessionRuntimeProxyMode::Configured;
        state
    }

    fn server(state: &SessionRuntimeState, name: &str) -> SessionRuntimeServerState {
        state
            .servers
            .iter()
            .find(|server| server.name == name)
            .cloned()
            .unwrap()
    }

    #[test]
    fn a_slow_server_becomes_ready_after_the_session_stopped_waiting() {
        let mut state = configured_state();
        apply_server_connecting(&mut state, "slow", SessionRuntimeTransport::Stdio);
        apply_server_ready(&mut state, "fast", SessionRuntimeTransport::Stdio, 2);

        // The session wait runs out: the slow server is TimedOut, the session is Ready.
        assert!(apply_discovery_timeout_finalize(&mut state, "waited 30s"));
        assert_eq!(
            server(&state, "slow").status,
            SessionRuntimeServerStatus::TimedOut
        );
        assert!(state.proxy.ready);

        // The server finishes loading later: its tools show up and it is no longer TimedOut.
        apply_server_ready(&mut state, "slow", SessionRuntimeTransport::Stdio, 5);
        apply_initialization_complete(&mut state);
        let slow = server(&state, "slow");
        assert_eq!(slow.status, SessionRuntimeServerStatus::Ready);
        assert_eq!(slow.tool_count, 5);
        assert_eq!(slow.error, None);
        assert_eq!(
            state.initialization.result,
            SessionRuntimeInitResult::Success
        );
        assert!(state.proxy.ready);
    }

    #[test]
    fn a_session_whose_only_server_timed_out_is_not_failed() {
        let mut state = configured_state();
        apply_server_connecting(&mut state, "slow", SessionRuntimeTransport::Stdio);
        apply_discovery_timeout_finalize(&mut state, "waited 30s");

        assert_eq!(state.phase, SessionRuntimePhase::Ready);
        assert_eq!(
            state.initialization.result,
            SessionRuntimeInitResult::Success
        );
        assert_eq!(state.initialization.error, None);
        assert!(state.proxy.ready);
        assert_eq!(
            state.initialization.current_step.as_deref(),
            Some("MCP still starting: slow")
        );

        apply_server_ready(&mut state, "slow", SessionRuntimeTransport::Stdio, 3);
        apply_initialization_complete(&mut state);
        assert_eq!(
            state.initialization.result,
            SessionRuntimeInitResult::Success
        );
        assert_eq!(server(&state, "slow").tool_count, 3);
    }

    #[test]
    fn several_timed_out_servers_all_join_later_without_a_failed_session() {
        let mut state = configured_state();
        for name in ["fred", "jupyter", "yahoo_finance"] {
            apply_server_connecting(&mut state, name, SessionRuntimeTransport::Stdio);
        }
        apply_discovery_timeout_finalize(&mut state, "waited 30s");
        assert_eq!(state.phase, SessionRuntimePhase::Ready);
        assert_eq!(state.initialization.error, None);

        for name in ["fred", "jupyter", "yahoo_finance"] {
            apply_server_ready(&mut state, name, SessionRuntimeTransport::Stdio, 1);
        }
        apply_initialization_complete(&mut state);
        assert_eq!(state.phase, SessionRuntimePhase::Ready);
        assert_eq!(
            state.initialization.result,
            SessionRuntimeInitResult::Success
        );
    }

    #[test]
    fn a_failed_server_is_still_counted_next_to_a_timed_out_one() {
        let mut state = configured_state();
        apply_server_connecting(&mut state, "slow", SessionRuntimeTransport::Stdio);
        apply_server_failed(
            &mut state,
            "broken",
            SessionRuntimeTransport::Stdio,
            "process exited".to_string(),
        );
        apply_discovery_timeout_finalize(&mut state, "waited 30s");

        assert_eq!(state.phase, SessionRuntimePhase::Degraded);
        assert_eq!(
            state.initialization.result,
            SessionRuntimeInitResult::Partial
        );
        assert_eq!(
            state.initialization.error.as_deref(),
            Some("1 of 2 external servers failed during initialization")
        );
        assert!(state.proxy.ready);
        assert_eq!(
            server(&state, "slow").status,
            SessionRuntimeServerStatus::TimedOut
        );
    }

    #[test]
    fn a_server_that_fails_is_failed_immediately_not_timed_out() {
        let mut state = configured_state();
        apply_server_connecting(&mut state, "broken", SessionRuntimeTransport::Stdio);
        apply_server_failed(
            &mut state,
            "broken",
            SessionRuntimeTransport::Stdio,
            "process exited".to_string(),
        );

        let broken = server(&state, "broken");
        assert_eq!(broken.status, SessionRuntimeServerStatus::Failed);
        assert_eq!(broken.error.as_deref(), Some("process exited"));

        // The session wait running out later must not rewrite a real failure.
        apply_discovery_timeout_finalize(&mut state, "waited 30s");
        assert_eq!(
            server(&state, "broken").status,
            SessionRuntimeServerStatus::Failed
        );
    }

    #[test]
    fn a_retry_claims_failed_servers_once() {
        let mut state = configured_state();
        apply_server_failed(
            &mut state,
            "broken",
            SessionRuntimeTransport::Stdio,
            "x".into(),
        );

        let first = claim_failed_servers_for_retry(&mut state, |_| true);
        assert_eq!(first, vec!["broken".to_string()]);
        assert_eq!(
            server(&state, "broken").status,
            SessionRuntimeServerStatus::Connecting
        );
        assert_eq!(server(&state, "broken").error.as_deref(), Some("x"));

        // A second trigger for the same server finds nothing to retry.
        assert!(claim_failed_servers_for_retry(&mut state, |_| true).is_empty());
    }

    #[test]
    fn a_retry_keeps_the_last_failure_reason_until_ready_or_a_new_failure() {
        let mut state = configured_state();
        apply_server_failed(
            &mut state,
            "broken",
            SessionRuntimeTransport::Stdio,
            "image pull failed".into(),
        );

        claim_failed_servers_for_retry(&mut state, |_| true);
        apply_server_connecting(&mut state, "broken", SessionRuntimeTransport::Stdio);
        apply_server_discovering(&mut state, "broken", SessionRuntimeTransport::Stdio);
        assert_eq!(
            server(&state, "broken").error.as_deref(),
            Some("image pull failed")
        );

        apply_server_failed(
            &mut state,
            "broken",
            SessionRuntimeTransport::Stdio,
            "process exited".into(),
        );
        assert_eq!(
            server(&state, "broken").error.as_deref(),
            Some("process exited")
        );

        claim_failed_servers_for_retry(&mut state, |_| true);
        apply_server_ready(&mut state, "broken", SessionRuntimeTransport::Stdio, 2);
        assert_eq!(server(&state, "broken").error, None);
    }

    #[test]
    fn a_retry_leaves_servers_that_are_not_failed_alone() {
        let mut state = configured_state();
        apply_server_connecting(&mut state, "slow", SessionRuntimeTransport::Stdio);
        apply_server_ready(&mut state, "ready", SessionRuntimeTransport::Stdio, 1);
        apply_server_failed(
            &mut state,
            "broken",
            SessionRuntimeTransport::Stdio,
            "x".into(),
        );
        // The session stopped waiting for "slow", which is still starting in the background.
        assert!(apply_discovery_timeout_finalize(&mut state, "waited 30s"));
        assert_eq!(
            server(&state, "slow").status,
            SessionRuntimeServerStatus::TimedOut
        );

        let claimed = claim_failed_servers_for_retry(&mut state, |_| true);

        assert_eq!(claimed, vec!["broken".to_string()]);
        assert_eq!(
            server(&state, "slow").status,
            SessionRuntimeServerStatus::TimedOut
        );
        assert_eq!(
            server(&state, "ready").status,
            SessionRuntimeServerStatus::Ready
        );
    }

    #[test]
    fn a_retry_only_claims_servers_it_has_a_config_for() {
        let mut state = configured_state();
        apply_server_failed(&mut state, "a", SessionRuntimeTransport::Stdio, "x".into());
        apply_server_failed(&mut state, "b", SessionRuntimeTransport::Stdio, "x".into());

        let claimed = claim_failed_servers_for_retry(&mut state, |name| name == "b");

        assert_eq!(claimed, vec!["b".to_string()]);
        assert_eq!(
            server(&state, "a").status,
            SessionRuntimeServerStatus::Failed
        );
    }

    #[test]
    fn a_discovery_task_that_died_leaves_failed_servers_not_timed_out_ones() {
        let mut state = configured_state();
        apply_server_connecting(&mut state, "starting", SessionRuntimeTransport::Stdio);
        apply_server_ready(&mut state, "ready", SessionRuntimeTransport::Stdio, 1);

        apply_discovery_task_failed(&mut state, "Tool discovery task failed before completion");

        let starting = server(&state, "starting");
        assert_eq!(starting.status, SessionRuntimeServerStatus::Failed);
        assert_eq!(
            starting.error.as_deref(),
            Some("Tool discovery task failed before completion")
        );
        assert_eq!(
            server(&state, "ready").status,
            SessionRuntimeServerStatus::Ready
        );
        // And it is a retry target.
        assert_eq!(
            claim_failed_servers_for_retry(&mut state, |_| true),
            vec!["starting".to_string()]
        );
    }

    #[test]
    fn a_task_that_dies_after_the_deadline_turns_timed_out_servers_into_retry_targets() {
        let mut state = configured_state();
        apply_server_connecting(&mut state, "slow", SessionRuntimeTransport::Stdio);
        apply_server_ready(&mut state, "ready", SessionRuntimeTransport::Stdio, 1);
        apply_discovery_timeout_finalize(&mut state, "waited 30s");
        assert_eq!(
            server(&state, "slow").status,
            SessionRuntimeServerStatus::TimedOut
        );

        apply_discovery_task_failed(&mut state, "Tool discovery task failed before completion");

        assert_eq!(
            server(&state, "slow").status,
            SessionRuntimeServerStatus::Failed
        );
        assert_eq!(
            server(&state, "ready").status,
            SessionRuntimeServerStatus::Ready
        );
        assert_eq!(
            claim_failed_servers_for_retry(&mut state, |_| true),
            vec!["slow".to_string()]
        );
    }
}
