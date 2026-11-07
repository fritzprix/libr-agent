use super::*;
use std::time::{Duration, Instant};

/// Issue #2633: explicit terminate must fence a session against late tool
/// dispatches even when no proxy exists yet, so a late callback can never
/// lazily recreate builtin instances after teardown.
#[tokio::test]
async fn terminate_without_proxy_fences_late_call_tool() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "terminate-fence-no-proxy";

    insert_test_session(manager, session_id).await;

    // No proxy exists yet: nothing to kill, but the fence must still be set.
    let killed = manager.terminate_session_resources(session_id).await;
    assert_eq!(killed, 0, "no proxy means no killable resources");
    assert!(
        manager.get_proxy(session_id).await.is_none(),
        "terminate must not create a proxy"
    );

    let error = manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect_err("fenced session must reject call_tool");
    assert!(
        error.contains("terminated"),
        "unexpected fence error: {error}"
    );
    assert!(
        manager.get_proxy(session_id).await.is_none(),
        "rejected call_tool must not lazily recreate a proxy"
    );
}

/// The fence rejects every late dispatch (no lazy revival), and only an
/// explicit lifecycle entry point (`clear_teardown_fence`, called by proxy
/// bootstrap, an explicit user/API message, and session reload) re-enables
/// tool calls.
#[tokio::test]
async fn fence_rejects_repeated_dispatch_until_lifecycle_entry_clears_it() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "terminate-fence-recreate";

    insert_test_session(manager, session_id).await;

    // A normal tool call lazily creates the builtin proxy.
    manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect("first call_tool should succeed");
    assert!(manager.get_proxy(session_id).await.is_some());

    // Terminate force-stops resources (idempotent when called twice).
    let killed = manager.terminate_session_resources(session_id).await;
    assert_eq!(killed, 0, "playbook server owns no processes");
    let killed_again = manager.terminate_session_resources(session_id).await;
    assert_eq!(killed_again, 0, "terminate must be idempotent");

    // Every late dispatch is rejected; no lazy revival happens.
    for _ in 0..3 {
        let error = manager
            .call_tool(session_id, "playbook__listPlaybooks", json!({}))
            .await
            .expect_err("fenced call must be rejected");
        assert!(error.contains("terminated"), "unexpected error: {error}");
    }

    // A new lifecycle entry (session create / explicit message / reload) clears
    // the fence, and the session can work again.
    manager.clear_teardown_fence(session_id).await;
    manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect("call_tool must work again after the fence is cleared");
}

/// The fence is keyed by session id: terminating one session must not fence a
/// bystander session's tool calls.
#[tokio::test]
async fn fence_is_session_scoped() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let terminated = "fence-target";
    let other = "fence-bystander";

    insert_test_session(manager, terminated).await;
    insert_test_session(manager, other).await;

    manager
        .call_tool(other, "playbook__listPlaybooks", json!({}))
        .await
        .expect("bystander call_tool should succeed");

    manager.terminate_session_resources(terminated).await;

    assert!(manager
        .call_tool(terminated, "playbook__listPlaybooks", json!({}))
        .await
        .is_err());
    manager
        .call_tool(other, "playbook__listPlaybooks", json!({}))
        .await
        .expect("other session must not be fenced");
}

/// Soft cancel (Stop/cancel_workflow) must never fence: the workflow keeps
/// running after a stopped tool, so its session stays usable.
#[tokio::test]
async fn soft_cancel_never_fences_the_session() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "cancel-never-fences";

    insert_test_session(manager, session_id).await;

    manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect("setup call_tool should succeed");

    manager
        .kill_session_processes(session_id)
        .await
        .expect("soft cancel must succeed");

    assert!(
        !manager.is_teardown_fenced(session_id).await,
        "soft cancel must not fence the session"
    );
    manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect("tools must keep working after a soft cancel");
}

/// Automatic wakes (MCP channel notification, scheduled job, compaction
/// continuation, background resume) all route through `ensure_proxy_ready` →
/// `ensure_proxy_exists`, which refuses a fenced session. This drives the real
/// product entry point (no AppHandle needed) rather than the manager helper.
/// An explicit user or API message clears the fence first, so it is accepted.
#[tokio::test]
async fn automatic_wake_is_rejected_until_an_explicit_message_clears_the_fence() {
    use crate::agent::workflow::start::ensure_proxy_exists;

    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "wake-rejection";

    insert_test_session(manager, session_id).await;

    // Not fenced yet: an ordinary wake passes and may create the proxy.
    ensure_proxy_exists(manager, None, session_id)
        .await
        .expect("a live session must be wakeable");

    // The real terminate path force-stops resources and then tears the proxy
    // down; the fence stays set afterwards, so late wakes cannot revive it.
    manager.terminate_session_resources(session_id).await;
    manager.destroy_proxy(session_id).await;

    let error = ensure_proxy_exists(manager, None, session_id)
        .await
        .expect_err("a terminated session must refuse automatic wakes");
    assert!(error.contains("terminated"), "unexpected error: {error}");
    assert!(
        crate::mcp::MCPServiceProxyManager::is_teardown_fence_rejection(&error),
        "wake refusal must carry the fence rejection marker"
    );
    assert!(
        manager.get_proxy(session_id).await.is_none(),
        "a refused wake must not recreate the proxy"
    );

    // The explicit message entry points clear the fence before starting work.
    manager.clear_teardown_fence(session_id).await;
    ensure_proxy_exists(manager, None, session_id)
        .await
        .expect("an explicit later message must be accepted");
}

/// Deterministically parks one lazy builtin-proxy creation for `session_id`
/// and starts a `call_tool` that reaches it. Waits until creation is parked
/// inside the creation guard (after the double-check), then returns the
/// dispatch task and the release signal, so the test can terminate while
/// creation is in-flight (issue #2633 G2.1).
async fn start_held_lazy_creation(
    manager: &Arc<MCPServiceProxyManager>,
    session_id: &'static str,
) -> (
    tokio::task::JoinHandle<Result<crate::mcp::types::MCPResponse, String>>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    manager
        .lazy_creation_holds
        .lock()
        .await
        .insert(session_id.to_string(), (ready_tx, release_rx));

    let calling_manager = manager.clone();
    let task = tokio::spawn(async move {
        calling_manager
            .call_tool(session_id, "playbook__listPlaybooks", json!({}))
            .await
    });
    ready_rx.await.expect("creation must reach the hold");
    (task, release_tx)
}

/// Issue #2633 G2.1: explicit terminate while a lazy proxy creation is
/// in-flight must reject that dispatch when it resumes — the parked creation
/// may not resurrect builtin instances after teardown.
#[tokio::test]
async fn terminate_during_inflight_lazy_creation_rejects_the_dispatch() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "terminate-during-creation";

    insert_test_session(manager, session_id).await;

    let (task, release) = start_held_lazy_creation(manager, session_id).await;

    let killed = manager.terminate_session_resources(session_id).await;
    assert_eq!(killed, 0, "no proxy finished yet, nothing to kill");
    assert!(manager.is_teardown_fenced(session_id).await);

    release.send(()).expect("release the parked creation");
    let error = task
        .await
        .expect("creation task must not panic")
        .expect_err("parked creation must be rejected after terminate");
    assert!(
        crate::mcp::MCPServiceProxyManager::is_teardown_fence_rejection(&error),
        "rejection must carry the fence marker, got: {error}"
    );
    assert!(
        manager.get_proxy(session_id).await.is_none(),
        "a terminated session must not end up with a recreated proxy"
    );
}

/// Issue #2633 G2.1: terminate followed by an explicit clear (resume/message)
/// while creation is still parked must STILL reject that in-flight dispatch.
/// Generation capture at entry detects the fence moving underneath it, so a
/// later clear cannot validate a dispatch that started before teardown.
#[tokio::test]
async fn clear_after_terminate_still_rejects_inflight_dispatch_by_generation() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "clear-after-terminate-during-creation";

    insert_test_session(manager, session_id).await;

    let (task, release) = start_held_lazy_creation(manager, session_id).await;

    manager.terminate_session_resources(session_id).await;
    // An explicit lifecycle entry clears the fence before creation resumes.
    manager.clear_teardown_fence(session_id).await;

    release.send(()).expect("release the parked creation");
    let error = task
        .await
        .expect("creation task must not panic")
        .expect_err("dispatch that captured the pre-terminate generation must be rejected");
    assert!(
        crate::mcp::MCPServiceProxyManager::is_teardown_fence_rejection(&error),
        "rejection must carry the fence marker, got: {error}"
    );

    // The session itself is usable again for NEW dispatches.
    manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect("new dispatch after explicit clear must succeed");
}

// ── Real process teardown (Linux) ───────────────────────────────────────────
//
// These tests spawn real OS processes through the production tool path and
// verify that explicit terminate really removes them. Windows runtime behavior
// is recorded as an unverified gap in the PR (issue #2633).

/// The child writes its own pid here. The shell `exec`s the final command, so
/// a command-line marker (`pgrep -f`) would be exec'd away; the pid file
/// survives the exec and identifies exactly the process teardown must kill.
#[cfg(unix)]
fn marker_pid_file(session_id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("issue2633-{}.pid", session_id))
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
async fn wait_for_pid_death(pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

/// Spawns a real background process through `workspace__spawnProcess` and
/// returns its OS pid once the child has reported it.
#[cfg(unix)]
async fn spawn_real_process(manager: &MCPServiceProxyManager, session_id: &str) -> u32 {
    let pid_file = marker_pid_file(session_id);
    let _ = std::fs::remove_file(&pid_file);

    manager
        .call_tool(
            session_id,
            "workspace__spawnProcess",
            json!({
                "command": format!("echo $$ > '{}' && sleep 60", pid_file.display()),
                "name": "fence-probe",
            }),
        )
        .await
        .expect("spawnProcess should succeed");

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                let _ = std::fs::remove_file(&pid_file);
                assert!(
                    pid_alive(pid),
                    "the spawned process ({pid}) should be running before teardown"
                );
                return pid;
            }
        }
        assert!(
            Instant::now() < deadline,
            "the spawned process never reported its pid via {}",
            pid_file.display()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Issue #2633 acceptance: explicit terminate must force-stop the session's
/// real spawned processes through production code, fence late dispatches, and
/// refuse automatic wakes.
#[cfg(unix)]
#[tokio::test]
async fn terminate_kills_real_session_processes_and_refuses_late_work() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "terminate-kills-real-process";

    insert_test_session(manager, session_id).await;

    let child_pid = spawn_real_process(manager, session_id).await;

    let killed = manager.terminate_session_resources(session_id).await;
    assert!(
        killed >= 1,
        "terminate must count the real spawned process, got {killed}"
    );

    assert!(
        wait_for_pid_death(child_pid).await,
        "process {child_pid} still alive after terminate"
    );

    // A late dispatch is refused and must not recreate anything usable.
    let error = manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect_err("fenced call must be rejected");
    assert!(error.contains("terminated"), "unexpected error: {error}");

    // And an automatic wake is refused too.
    let wake_error = manager
        .reject_if_teardown_fenced(session_id)
        .await
        .expect_err("fenced session must refuse automatic wakes");
    assert!(
        wake_error.contains("terminated"),
        "unexpected wake error: {wake_error}"
    );
}

/// Issue #2633: teardown is session-scoped for real processes too — another
/// session's live process must survive.
#[cfg(unix)]
#[tokio::test]
async fn terminate_does_not_kill_another_sessions_processes() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let target = "process-target";
    let bystander = "process-bystander";

    insert_test_session(manager, target).await;
    insert_test_session(manager, bystander).await;

    let target_pid = spawn_real_process(manager, target).await;
    let bystander_pid = spawn_real_process(manager, bystander).await;

    manager.terminate_session_resources(target).await;

    assert!(
        wait_for_pid_death(target_pid).await,
        "target process {target_pid} must be gone after teardown"
    );
    assert!(
        pid_alive(bystander_pid),
        "another session's process ({bystander_pid}) must survive teardown"
    );

    // Cleanup so the test does not leave a 60s sleep behind.
    manager.terminate_session_resources(bystander).await;
    assert!(
        wait_for_pid_death(bystander_pid).await,
        "cleanup terminate must remove the bystander process {bystander_pid}"
    );
}

/// Issue #2633 G3.1: `resume_session_and_workflow` (used by the explicit
/// `agent_resume_workflow` command and `POST /api/sessions/:id/resume`) must
/// lift a force-stop fence before resuming, so terminate followed by an
/// explicit resume runs instead of being refused into `Error`. The runtime
/// half — a fence refusal from `ensure_proxy_exists` that stops refusing once
/// `clear_teardown_fence` (what `allow_explicit_message_after_terminate`
/// calls) runs — is covered by
/// [`automatic_wake_is_rejected_until_an_explicit_message_clears_the_fence`].
/// A full `AgentSessionManager` harness needs a Tauri `AppHandle`, so the
/// wiring itself is pinned at the source level.
#[test]
fn resume_session_and_workflow_lifts_the_fence_before_resuming() {
    let source = include_str!("../../../agent/session_manager/mod.rs");
    let signature = source
        .find("pub async fn resume_session_and_workflow")
        .expect("resume_session_and_workflow must exist");
    let body = &source[signature..];
    let body = match body["pub async fn resume_session_and_workflow".len()..].find("pub async fn ")
    {
        Some(offset) => &body[..offset + "pub async fn resume_session_and_workflow".len()],
        None => body,
    };

    let clear = body
        .find("allow_explicit_message_after_terminate")
        .expect("resume_session_and_workflow must lift the force-stop fence (G3.1)");
    let resume = body
        .find("self.resume_session(")
        .expect("resume_session_and_workflow must still resume the session");
    assert!(
        clear < resume,
        "the fence must be lifted before resume starts, or the background \
         resume races the refusal"
    );
}

/// Issue #2633 G3.2: the background start (`start_workflow`) and background
/// resume (`resume_workflow`) must treat a force-stop fence refusal as a
/// correct outcome — same guard as background injection (G2.3) — instead of
/// marking the session `Error` and emitting `WorkflowError`. The runtime half
/// (the real refusal from `ensure_proxy_exists` carries the marker that
/// `is_teardown_fence_rejection` matches) is covered by
/// [`automatic_wake_is_rejected_until_an_explicit_message_clears_the_fence`];
/// spawning the real background tasks needs an `AppHandle`, so the guard
/// placement is pinned at the source level.
#[test]
fn background_start_and_resume_guard_fence_refusals_before_failing_the_session() {
    let cases = [
        (
            "start.rs",
            include_str!("../../../agent/workflow/start.rs"),
            "Proxy check failed during background start",
        ),
        (
            "pause_resume.rs",
            include_str!("../../../agent/workflow/pause_resume.rs"),
            "Proxy check failed during background resume",
        ),
    ];

    for (name, source, failure_log) in cases {
        let guard = source
            .find("is_teardown_fence_rejection")
            .unwrap_or_else(|| panic!("{name} must guard against fence refusals (G3.2)"));
        let failure = source
            .find(failure_log)
            .unwrap_or_else(|| panic!("{name} must keep the background failure path"));
        assert!(
            guard < failure,
            "{name}: the fence guard must run before the session is marked \
             failed and a WorkflowError is emitted"
        );
    }
}
