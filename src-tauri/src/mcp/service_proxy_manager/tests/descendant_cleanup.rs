//! Issue #2652: explicit terminate must kill a spawned wrapper *and* the
//! long-lived children it started, not only the wrapper pid.
#![cfg(unix)]

use super::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct WrapperPids {
    wrapper: u32,
    child: u32,
}

fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

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

fn read_pid(path: &PathBuf) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Spawns `bash` that backgrounds a long `sleep` and waits on it, so the
/// wrapper does not `exec` away and the child outlives a wrapper-only kill.
async fn spawn_wrapper_with_child(
    manager: &MCPServiceProxyManager,
    session_id: &str,
) -> WrapperPids {
    let dir = std::env::temp_dir();
    let child_file = dir.join(format!("issue2652-{session_id}.child"));
    let wrapper_file = dir.join(format!("issue2652-{session_id}.wrapper"));
    let _ = std::fs::remove_file(&child_file);
    let _ = std::fs::remove_file(&wrapper_file);

    manager
        .call_tool(
            session_id,
            "workspace__spawnProcess",
            json!({
                "command": format!(
                    "sleep 600 & echo $! > '{}'; echo $$ > '{}'; wait",
                    child_file.display(),
                    wrapper_file.display()
                ),
                "name": "descendant-probe",
            }),
        )
        .await
        .expect("spawnProcess should succeed");

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let (Some(child), Some(wrapper)) = (read_pid(&child_file), read_pid(&wrapper_file)) {
            let _ = std::fs::remove_file(&child_file);
            let _ = std::fs::remove_file(&wrapper_file);
            assert!(pid_alive(wrapper), "wrapper {wrapper} should be running");
            assert!(pid_alive(child), "child {child} should be running");
            return WrapperPids { wrapper, child };
        }
        assert!(
            Instant::now() < deadline,
            "the wrapper never reported its pids for session {session_id}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn assert_all_dead_and_stay_dead(pids: &WrapperPids) {
    assert!(
        wait_for_pid_death(pids.wrapper).await,
        "wrapper {} still alive after terminate",
        pids.wrapper
    );
    assert!(
        wait_for_pid_death(pids.child).await,
        "child {} of wrapper {} survived terminate",
        pids.child,
        pids.wrapper
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !pid_alive(pids.wrapper) && !pid_alive(pids.child),
        "a terminated process came back"
    );
}

#[tokio::test]
async fn terminate_kills_wrapper_and_its_long_lived_child() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "descendant-target";
    insert_test_session(manager, session_id).await;

    let pids = spawn_wrapper_with_child(manager, session_id).await;

    let killed = manager.terminate_session_resources(session_id).await;
    assert!(
        killed >= 1,
        "terminate must count the wrapper, got {killed}"
    );
    assert_all_dead_and_stay_dead(&pids).await;

    // Repeated cleanup is a harmless no-op.
    assert_eq!(manager.terminate_session_resources(session_id).await, 0);
}

#[tokio::test]
async fn terminate_kills_descendants_only_in_the_target_session() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let target = "descendant-scoped-target";
    let bystander = "descendant-scoped-bystander";
    insert_test_session(manager, target).await;
    insert_test_session(manager, bystander).await;

    let target_pids = spawn_wrapper_with_child(manager, target).await;
    let bystander_pids = spawn_wrapper_with_child(manager, bystander).await;

    manager.terminate_session_resources(target).await;
    assert_all_dead_and_stay_dead(&target_pids).await;
    assert!(
        pid_alive(bystander_pids.wrapper) && pid_alive(bystander_pids.child),
        "another session's wrapper and child must survive"
    );

    manager.terminate_session_resources(bystander).await;
    assert_all_dead_and_stay_dead(&bystander_pids).await;
}
