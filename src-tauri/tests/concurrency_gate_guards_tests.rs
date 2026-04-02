use std::time::Duration;

use tauri_mcp_agent_lib::agent::concurrency::ConcurrencyGate;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[tokio::test]
async fn suspended_guard_drop_releases_slots_without_leak() {
    let gate = Box::leak(Box::new(ConcurrencyGate::new(1, 1, 1, 1)));
    let active = gate.acquire_active_agent().await.unwrap();
    let suspended = gate.suspend_agent(active).await.unwrap();

    drop(suspended);

    // Use a generous timeout so slow CI runners (shared containers, high load)
    // don't flake. The semaphore notification is synchronous — the slot becomes
    // available immediately on drop, so 2 s is purely safety margin.
    let reacquired = timeout(Duration::from_secs(2), gate.acquire_active_agent()).await;
    assert!(
        reacquired.is_ok(),
        "dropping a suspended guard should release both suspended bookkeeping and the freed active slot"
    );
}

// Use multi_thread flavor so the spawned child task runs on its own OS thread
// and is never blocked by the main task holding the tokio current_thread reactor.
// With current_thread, the child's sleep future can never advance while the main
// task is parked inside semaphore::acquire_owned().await, causing an infinite hang
// on loaded CI runners where timer resolution is degraded.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspended_guard_resume_reacquires_active_slot() {
    let gate = Box::leak(Box::new(ConcurrencyGate::new(1, 1, 1, 1)));
    let child_gate: &'static ConcurrencyGate = gate;
    let active = gate.acquire_active_agent().await.unwrap();
    let (acquired_tx, acquired_rx) = oneshot::channel();

    let child = {
        tokio::spawn(async move {
            let permit = child_gate.acquire_active_agent().await.unwrap();
            let _ = acquired_tx.send(());
            // Hold the slot briefly so the parent's resume() must truly wait.
            tokio::time::sleep(Duration::from_millis(50)).await;
            drop(permit);
        })
    };

    let suspended = gate.suspend_agent(active).await.unwrap();
    // Give the child 5 s to acquire the freed active slot — generous for slow CI.
    timeout(Duration::from_secs(5), acquired_rx)
        .await
        .expect("child should acquire the freed active slot while parent is suspended")
        .expect("child acquisition signal should be sent");
    // 5 s for resume(): child holds the slot for ~50 ms then drops, so well within budget.
    let resumed = timeout(Duration::from_secs(5), suspended.resume()).await;
    assert!(
        resumed.is_ok(),
        "resume should wait for a free active slot and reacquire it"
    );

    child.await.unwrap();
}
