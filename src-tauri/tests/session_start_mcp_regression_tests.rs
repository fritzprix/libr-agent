//! Regression guards for session start vs MCP discovery failures.
//!
//! These are pure/source-level tests so they run on Windows CI targets too
//! (the consolidated `integration_tests` binary is Linux/macOS-only).

/// Hard-gating `agent_send_message` on `proxy.ready` made draft session start
/// fail whenever any MCP server was still initializing or degraded mid-load.
/// Discovery must complete in the background; failures surface via runtime-state
/// Sonner toasts instead of rejecting the send command.
#[test]
fn agent_send_message_must_not_hard_reject_unready_proxy() {
    let src = include_str!("../src/commands/agent_commands/workflow_commands.rs");
    assert!(
        !src.contains("MCP proxy not ready"),
        "agent_send_message must not reject session start while MCP discovery is in progress; \
         start_workflow already waits via ensure_proxy_ready in the background"
    );
    assert!(
        src.contains("start_workflow"),
        "agent_send_message must still delegate to start_workflow"
    );
}

/// Failed external MCP must not permanently disable chat when a proxy exists
/// (builtins remain usable). Guard the readiness rule in runtime_state.
#[test]
fn failed_external_mcp_keeps_proxy_ready_when_proxy_exists() {
    use tauri_mcp_agent_lib::agent::runtime_state::{
        SessionRuntimeInitResult, SessionRuntimePhase, SessionRuntimeServerState,
        SessionRuntimeServerStatus, SessionRuntimeState, SessionRuntimeTransport,
    };

    let mut state = SessionRuntimeState::configured_initializing(vec![SessionRuntimeServerState {
        name: "broken".to_string(),
        transport: SessionRuntimeTransport::Stdio,
        status: SessionRuntimeServerStatus::NotStarted,
        tool_count: 0,
        error: None,
    }]);
    state.set_proxy_exists(true);
    state.upsert_server(
        "broken",
        SessionRuntimeTransport::Stdio,
        SessionRuntimeServerStatus::Failed,
        0,
        Some("connection closed".to_string()),
    );
    state.recompute_summary();

    assert_eq!(state.phase, SessionRuntimePhase::Failed);
    assert_eq!(
        state.initialization.result,
        SessionRuntimeInitResult::Failed
    );
    assert!(
        state.proxy.ready,
        "proxy.exists=true + all external MCP failed must still set ready so session chat can start"
    );
}
