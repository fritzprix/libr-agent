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
        Some("1 of 2 external servers failed during initialization")
    );
    assert!(state.proxy.ready);
}
