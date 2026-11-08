//! Issue #2633 acceptance counterweight: normal completion → idle must NOT
//! behave like terminate. A background process, the session's wake token,
//! the teardown fence state, and the proxy itself all survive the normal
//! idle transition; only explicit terminate/delete force-stops them.
#![cfg(unix)]

use super::product_entry::{entry_deps, set_parent};
use super::termination_fence::{pid_alive, spawn_real_process, wait_for_pid_death};
use super::*;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::state::AgentSession;
use crate::repositories::{SessionMetadata, SessionStatus};
use serde_json::json;
use std::collections::HashMap;

/// The in-memory session that `apply_session_status` (the finish path's idle
/// transition) requires, loaded exactly as the product loads a session.
fn active_session(session_id: &str) -> Arc<RwLock<HashMap<String, AgentSession>>> {
    let session = AgentSession::new(
        SessionMetadata::test_fixture(session_id),
        Arc::new(ContextRegistry::new()),
        None,
    );
    Arc::new(RwLock::new(HashMap::from([(
        session_id.to_string(),
        session,
    )])))
}

#[tokio::test]
async fn normal_idle_transition_preserves_process_wake_fence_and_proxy() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let deps = entry_deps(manager);
    let session_id = "idle-preserve-normal";

    insert_test_session(manager, session_id).await;
    let pid = spawn_real_process(manager, session_id).await;
    let token = crate::server::wake::wake_token_for(session_id);
    let active_sessions = active_session(session_id);

    // The exact transition `finish.rs` performs when a workflow completes
    // normally (`transition_to_terminal_status_and_emit` → this function).
    // No AppHandle is available here, so the finish caller is pinned
    // statically in `finish_path_never_force_stops`.
    crate::agent::lifecycle::update_session_status_with_dispatcher(
        &deps.session_repo,
        &active_sessions,
        &deps.dispatcher,
        session_id,
        SessionStatus::Idle,
    )
    .await
    .expect("idle transition must succeed");

    assert!(
        pid_alive(pid),
        "normal idle completion must not kill background process {pid}"
    );
    assert!(
        crate::server::wake::verify_wake_token(session_id, &token),
        "normal idle completion must not revoke the wake token"
    );
    assert!(
        !manager.is_teardown_fenced(session_id).await,
        "normal idle completion must not fence the session"
    );
    assert!(
        manager.get_proxy(session_id).await.is_some(),
        "normal idle completion must keep the proxy for delegate result delivery"
    );

    // Late work from an idle (not terminated) session is still accepted.
    manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect("an idle session must still accept tool calls");

    // Cleanup for the test itself, through the explicit terminate primitive.
    // Poll for death rather than checking liveness inline: `kill -0` still
    // succeeds on a killed-but-not-yet-reaped zombie (issue #2652 precedent).
    manager.terminate_session_resources(session_id).await;
    assert!(
        wait_for_pid_death(pid).await,
        "explicit terminate must kill background process {pid}"
    );
}

#[test]
fn finish_path_never_force_stops() {
    // The runtime test above pins the shared idle transition itself. This
    // pins the finish caller: the whole normal-completion module must never
    // reach into the explicit-terminate force-stop primitives.
    let source = include_str!("../../../agent/workflow/finish.rs");
    for forbidden in [
        "terminate_session_resources",
        "kill_session_processes",
        "destroy_proxy",
        "revoke_wake_token",
    ] {
        assert!(
            !source.contains(forbidden),
            "finish.rs must not call {forbidden}: normal completion is not terminate (#2633)"
        );
    }
}

#[tokio::test]
async fn idle_parent_keeps_delegate_wake_path_open() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let deps = entry_deps(manager);
    let parent = "idle-preserve-parent";
    let child = "idle-preserve-child";

    insert_test_session(manager, parent).await;
    insert_test_session(manager, child).await;
    set_parent(manager, child, parent).await;
    let child_pid = spawn_real_process(manager, child).await;
    let active_sessions = active_session(parent);

    // Parent completes normally; the child (delegate) keeps running and
    // reachable — an idle parent transition must not cascade a kill.
    crate::agent::lifecycle::update_session_status_with_dispatcher(
        &deps.session_repo,
        &active_sessions,
        &deps.dispatcher,
        parent,
        SessionStatus::Idle,
    )
    .await
    .expect("parent idle transition must succeed");

    assert!(
        pid_alive(child_pid),
        "idle parent completion must not kill the delegate process {child_pid}"
    );
    assert!(
        !manager.is_teardown_fenced(parent).await && !manager.is_teardown_fenced(child).await,
        "idle completion must not fence parent or child"
    );

    manager.terminate_session_resources(child).await;
    assert!(
        wait_for_pid_death(child_pid).await,
        "explicit terminate must kill the delegate process {child_pid}"
    );
}
