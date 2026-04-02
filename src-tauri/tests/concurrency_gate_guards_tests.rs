use std::time::Duration;

use tauri_mcp_agent_lib::agent::concurrency::ConcurrencyGate;
use tokio::time::timeout;

#[tokio::test]
async fn suspended_guard_drop_releases_slots_without_leak() {
    let gate = Box::leak(Box::new(ConcurrencyGate::new(1, 1, 1, 1)));
    let active = gate.acquire_active_agent().await.unwrap();
    let suspended = gate.suspend_agent(active).await.unwrap();

    drop(suspended);

    let reacquired = timeout(Duration::from_millis(50), gate.acquire_active_agent()).await;
    assert!(
        reacquired.is_ok(),
        "dropping a suspended guard should release both suspended bookkeeping and the freed active slot"
    );
}

#[tokio::test]
async fn suspended_guard_resume_reacquires_active_slot() {
    let gate = Box::leak(Box::new(ConcurrencyGate::new(1, 1, 1, 1)));
    let child_gate: &'static ConcurrencyGate = gate;
    let active = gate.acquire_active_agent().await.unwrap();

    let child = {
        tokio::spawn(async move {
            let permit = child_gate.acquire_active_agent().await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            drop(permit);
        })
    };

    let suspended = gate.suspend_agent(active).await.unwrap();
    let resumed = timeout(Duration::from_millis(100), suspended.resume()).await;
    assert!(
        resumed.is_ok(),
        "resume should wait for a free active slot and reacquire it"
    );

    child.await.unwrap();
}
