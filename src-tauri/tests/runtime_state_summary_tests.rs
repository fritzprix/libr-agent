//! Windows-safe unit tests for session runtime summary / discovery finalize.
//! (Not behind cfg(not(windows)) — no Tauri WebView link.)

use tauri_mcp_agent_lib::agent::runtime_state::{
    SessionRuntimeInitResult, SessionRuntimePhase, SessionRuntimeServerState,
    SessionRuntimeServerStatus, SessionRuntimeState, SessionRuntimeTransport,
};

fn configured_state() -> SessionRuntimeState {
    SessionRuntimeState::configured_initializing(vec![
        SessionRuntimeServerState {
            name: "alpha".to_string(),
            transport: SessionRuntimeTransport::Stdio,
            status: SessionRuntimeServerStatus::NotStarted,
            tool_count: 0,
            error: None,
        },
        SessionRuntimeServerState {
            name: "beta".to_string(),
            transport: SessionRuntimeTransport::Http,
            status: SessionRuntimeServerStatus::NotStarted,
            tool_count: 0,
            error: None,
        },
    ])
}

#[test]
fn server_progress_does_not_finalize_summary_mid_bootstrap() {
    let mut state = configured_state();

    state.upsert_server(
        "alpha",
        SessionRuntimeTransport::Stdio,
        SessionRuntimeServerStatus::Failed,
        0,
        Some("alpha failed".to_string()),
    );
    state.upsert_server(
        "beta",
        SessionRuntimeTransport::Http,
        SessionRuntimeServerStatus::Ready,
        3,
        None,
    );

    assert_eq!(
        state.phase,
        SessionRuntimePhase::Initializing,
        "per-server progress should not finalize the session summary before bootstrap completes"
    );
    assert_eq!(
        state.initialization.result,
        SessionRuntimeInitResult::Pending,
        "initialization result should stay pending until finalization"
    );
}

#[test]
fn recompute_summary_finalizes_to_degraded_after_bootstrap() {
    let mut state = configured_state();

    state.upsert_server(
        "alpha",
        SessionRuntimeTransport::Stdio,
        SessionRuntimeServerStatus::Failed,
        0,
        Some("alpha failed".to_string()),
    );
    state.upsert_server(
        "beta",
        SessionRuntimeTransport::Http,
        SessionRuntimeServerStatus::Ready,
        3,
        None,
    );

    state.recompute_summary();

    assert_eq!(state.phase, SessionRuntimePhase::Degraded);
    assert_eq!(
        state.initialization.result,
        SessionRuntimeInitResult::Partial
    );
    assert_eq!(
        state.initialization.error.as_deref(),
        Some("1 of 2 external servers failed or timed out during initialization")
    );
    assert!(state.proxy.ready);
}

#[test]
fn finalize_discovery_timeout_marks_pending_servers_and_sets_ready() {
    let mut state = configured_state();
    state.set_proxy_exists(true);

    state.upsert_server(
        "alpha",
        SessionRuntimeTransport::Stdio,
        SessionRuntimeServerStatus::Connecting,
        0,
        None,
    );
    state.upsert_server(
        "beta",
        SessionRuntimeTransport::Http,
        SessionRuntimeServerStatus::Ready,
        2,
        None,
    );

    assert!(state.finalize_discovery_timeout("discovery soft timeout"));

    assert_eq!(
        state.servers[0].status,
        SessionRuntimeServerStatus::TimedOut
    );
    assert_eq!(
        state.servers[0].error.as_deref(),
        Some("discovery soft timeout")
    );
    assert_eq!(state.servers[1].status, SessionRuntimeServerStatus::Ready);
    assert_eq!(state.phase, SessionRuntimePhase::Degraded);
    assert_eq!(
        state.initialization.result,
        SessionRuntimeInitResult::Partial
    );
    assert!(state.proxy.ready);
}

#[test]
fn finalize_discovery_timeout_is_idempotent_after_bootstrap() {
    let mut state = configured_state();
    state.set_proxy_exists(true);

    state.upsert_server(
        "alpha",
        SessionRuntimeTransport::Stdio,
        SessionRuntimeServerStatus::Failed,
        0,
        Some("boom".to_string()),
    );
    state.upsert_server(
        "beta",
        SessionRuntimeTransport::Http,
        SessionRuntimeServerStatus::Ready,
        1,
        None,
    );
    state.recompute_summary();

    assert!(!state.finalize_discovery_timeout("late waiter timeout"));
    assert_eq!(state.phase, SessionRuntimePhase::Degraded);
    assert!(state.proxy.ready);
}

#[test]
fn finalize_discovery_timeout_all_pending_becomes_failed_but_ready_with_proxy() {
    let mut state = configured_state();
    state.set_proxy_exists(true);

    assert!(state.finalize_discovery_timeout("deadline exceeded"));

    assert!(state
        .servers
        .iter()
        .all(|s| s.status == SessionRuntimeServerStatus::TimedOut));
    assert_eq!(state.phase, SessionRuntimePhase::Failed);
    assert_eq!(
        state.initialization.result,
        SessionRuntimeInitResult::Failed
    );
    assert!(state.proxy.ready);
}

#[test]
fn recompute_summary_all_external_failures_keeps_proxy_ready_when_exists() {
    let mut state = configured_state();
    state.set_proxy_exists(true);

    state.upsert_server(
        "alpha",
        SessionRuntimeTransport::Stdio,
        SessionRuntimeServerStatus::Failed,
        0,
        Some("alpha failed".to_string()),
    );
    state.upsert_server(
        "beta",
        SessionRuntimeTransport::Http,
        SessionRuntimeServerStatus::Failed,
        0,
        Some("beta failed".to_string()),
    );

    state.recompute_summary();

    assert_eq!(state.phase, SessionRuntimePhase::Failed);
    assert_eq!(
        state.initialization.result,
        SessionRuntimeInitResult::Failed
    );
    assert!(
        state.proxy.ready,
        "session must remain usable with builtin tools when proxy exists"
    );
}

#[test]
fn recompute_summary_all_external_failures_without_proxy_stays_not_ready() {
    let mut state = configured_state();
    assert!(!state.proxy.exists);

    state.upsert_server(
        "alpha",
        SessionRuntimeTransport::Stdio,
        SessionRuntimeServerStatus::Failed,
        0,
        Some("alpha failed".to_string()),
    );
    state.upsert_server(
        "beta",
        SessionRuntimeTransport::Http,
        SessionRuntimeServerStatus::Failed,
        0,
        Some("beta failed".to_string()),
    );

    state.recompute_summary();

    assert_eq!(state.phase, SessionRuntimePhase::Failed);
    assert!(
        !state.proxy.ready,
        "without a proxy there are no tools to use"
    );
}
