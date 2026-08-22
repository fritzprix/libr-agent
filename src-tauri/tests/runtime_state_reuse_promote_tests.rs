//! Windows-safe pure tests for builtin Reuse runtime promotion (no Tauri/WebView link).

use tauri_mcp_agent_lib::agent::runtime_state::{SessionRuntimePhase, SessionRuntimeState};
use tauri_mcp_agent_lib::mcp::service_proxy_manager::promote_builtin_reuse_runtime_state;

#[test]
fn promote_builtin_reuse_makes_unready_ready_without_external_servers() {
    let not_ready = SessionRuntimeState::default();
    assert!(!not_ready.proxy.ready);

    let promoted = promote_builtin_reuse_runtime_state(not_ready, false);
    assert!(promoted.proxy.ready);
    assert_eq!(promoted.phase, SessionRuntimePhase::Ready);
}

#[test]
fn promote_builtin_reuse_keeps_ready_snapshot() {
    let ready = SessionRuntimeState::builtin_ready();
    let kept = promote_builtin_reuse_runtime_state(ready.clone(), false);
    assert_eq!(kept, ready);
}

#[test]
fn promote_builtin_reuse_does_not_force_ready_with_external_servers() {
    let unready = SessionRuntimeState::default();
    let unchanged = promote_builtin_reuse_runtime_state(unready.clone(), true);
    assert_eq!(unchanged, unready);
    assert!(!unchanged.proxy.ready);
}
