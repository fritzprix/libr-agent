use super::*;

#[test]
fn test_decide_proxy_readiness_state_truth_table() {
    // Missing Proxy
    assert_eq!(
        decide_proxy_readiness_state(false, false),
        ProxyReadinessState::MissingProxy
    );
    assert_eq!(
        decide_proxy_readiness_state(false, true),
        ProxyReadinessState::MissingProxy
    );

    // Proxy exists, no readiness signal -> Immediately Ready
    assert_eq!(
        decide_proxy_readiness_state(true, false),
        ProxyReadinessState::Ready
    );

    // Proxy exists, has readiness signal -> Must await signal
    assert_eq!(
        decide_proxy_readiness_state(true, true),
        ProxyReadinessState::AwaitSignal
    );
}

// ── Regression tests for proxy_readiness / wait_until_proxy_ready ─────────────

/// Regression: builtin-only sessions must be considered immediately ready.
///
/// Before the fix, if `start_workflow` called `wait_until_proxy_ready` for a
/// session with no external servers it would find "no entry" in the map.
/// The method should treat that as ready and return `Ok(())` without blocking.
#[tokio::test]
async fn test_proxy_ready_immediately_for_builtin_only_sessions() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-builtin-only";

    insert_test_session(&manager, session_id).await;

    // No external MCP server IDs → background task is never spawned.
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["setup-wizard".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();

    // Must resolve within a tight wall-clock budget (100 ms) — it should be instant.
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        manager.wait_until_proxy_ready(session_id, 10),
    )
    .await;

    assert!(
        result.is_ok(),
        "wait_until_proxy_ready must not block for builtin-only sessions"
    );
    assert!(
        result.unwrap().is_ok(),
        "wait_until_proxy_ready must succeed"
    );

    // No entry should exist in the readiness map.
    assert_eq!(
        manager.readiness_entry_count().await,
        0,
        "Builtin-only sessions must not add an entry to proxy_readiness"
    );

    manager.destroy_proxy(session_id).await;
}

/// Regression: wait_until_proxy_ready must reject missing proxies.
///
/// A missing readiness entry only means "builtin-only and already ready" when a
/// proxy actually exists. Destroyed or never-created sessions must not be treated
/// as ready, or workflow code will race into LLM/tool execution without external
/// MCP managers.
#[tokio::test]
async fn test_wait_fails_when_proxy_does_not_exist() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-missing-proxy";

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        manager.wait_until_proxy_ready(session_id, 1),
    )
    .await;

    assert!(
        result.is_ok(),
        "wait_until_proxy_ready must fail fast when no proxy exists"
    );
    assert!(
        result
            .unwrap()
            .is_err_and(|error| error.contains("No MCP proxy exists")),
        "wait_until_proxy_ready must report the missing proxy instead of pretending the session is ready"
    );
}

/// Regression: wait_until_proxy_ready must block until the background signal fires.
///
/// This verifies the core race-condition fix: a session with external MCP servers
/// has a pending readiness entry. start_workflow must wait until tool loading
/// completes before proceeding.
#[tokio::test]
async fn test_wait_blocks_until_ready_signal_fires() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-signal-test";

    insert_test_session(&manager, session_id).await;
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();
    manager
        .mark_runtime_proxy_not_ready_for_test(session_id)
        .await;

    // Inject a pending (false) entry simulating a session with external servers
    // whose background loading has not finished yet.
    let tx = manager.inject_pending_readiness_for_test(session_id).await;

    assert_eq!(
        manager.readiness_entry_count().await,
        1,
        "Pending readiness entry should exist"
    );

    // Spawn a task that signals ready after a short delay.
    let tx_bg = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        // send_replace updates even if no other receivers are currently subscribed.
        tx_bg.send_replace(true);
    });

    let start = std::time::Instant::now();
    let result = manager.wait_until_proxy_ready(session_id, 5).await;
    let elapsed = start.elapsed();

    assert!(
        result.is_ok(),
        "wait_until_proxy_ready must succeed after signal"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(40),
        "Must have waited for the signal (elapsed: {:?})",
        elapsed
    );
}

/// Regression: discovery completion must mark ready even when no waiter holds a
/// watch receiver (soft get_available_tools timeout dropped its subscription).
///
/// tokio::watch::Sender::send does not update the value when receiver_count == 0,
/// which previously left resume/start waiting on a forever-false signal.
#[tokio::test]
async fn test_ready_signal_survives_zero_receivers() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-zero-receivers";

    insert_test_session(&manager, session_id).await;
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();
    manager
        .mark_runtime_proxy_not_ready_for_test(session_id)
        .await;

    let tx = manager.inject_pending_readiness_for_test(session_id).await;

    // Reproduce production race: no live receivers when discovery completes.
    assert_eq!(tx.receiver_count(), 0);
    assert!(
        tx.send(true).is_err(),
        "watch::send must fail with zero receivers (tokio contract)"
    );
    assert!(
        !*tx.borrow(),
        "failed send must leave the readiness value false"
    );

    // Production fix path.
    tx.send_replace(true);
    assert!(
        *tx.borrow(),
        "send_replace must publish ready with zero receivers"
    );

    let result = manager.wait_until_proxy_ready(session_id, 1).await;
    assert!(
        result.is_ok(),
        "wait must observe ready after send_replace with zero prior receivers"
    );
}

/// Regression: wait_until_proxy_ready must not block the session forever when a
/// readiness signal never fires. If a proxy already exists, finalize Session Ready
/// (TimedOut pending servers) so UI/backend Ready stay aligned.
#[tokio::test]
async fn test_wait_proceeds_degraded_if_never_signaled() {
    use crate::agent::runtime_state::{SessionRuntimeServerStatus, SessionRuntimeTransport};

    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-timeout-test";

    insert_test_session(&manager, session_id).await;
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();

    // Seed a non-terminal external server and clear Session Ready.
    {
        let mut state = manager.get_runtime_state(session_id).await;
        state.upsert_server(
            "slow-stdio",
            SessionRuntimeTransport::Stdio,
            SessionRuntimeServerStatus::DiscoveringTools,
            0,
            None,
        );
        state.proxy.ready = false;
        state.initialization.result =
            crate::agent::runtime_state::SessionRuntimeInitResult::Pending;
        state.phase = crate::agent::runtime_state::SessionRuntimePhase::Initializing;
        manager.set_runtime_state(session_id, state, None).await;
    }

    // Inject a pending entry but intentionally never send true.
    let _tx = manager.inject_pending_readiness_for_test(session_id).await;

    // Use a 1-second timeout so the test doesn't hang.
    let result = manager.wait_until_proxy_ready(session_id, 1).await;

    assert!(
        result.is_ok(),
        "Must proceed degraded when proxy exists but readiness never signals"
    );
    assert_eq!(
        manager.readiness_entry_count().await,
        0,
        "Timed-out readiness wait must be cleared so later calls do not re-block"
    );

    let state = manager.get_runtime_state(session_id).await;
    assert!(
        state.proxy.ready,
        "waiter timeout must raise Session Ready (proxy.ready)"
    );
    assert!(
        state.servers.iter().any(|server| {
            server.name == "slow-stdio" && server.status == SessionRuntimeServerStatus::TimedOut
        }),
        "pending servers must be marked TimedOut on waiter finalize"
    );
}

/// Without a proxy, readiness timeout remains a hard error (nothing to proceed with).
#[tokio::test]
async fn test_wait_errors_if_timed_out_without_proxy() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-timeout-no-proxy";

    insert_test_session(&manager, session_id).await;
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();

    let _tx = manager.inject_pending_readiness_for_test(session_id).await;

    let mgr_bg = manager.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        mgr_bg.destroy_proxy(session_id).await;
    });

    let result = manager.wait_until_proxy_ready(session_id, 1).await;

    assert!(result.is_err(), "Must error when no proxy exists");
    assert!(
        result.unwrap_err().contains("timed out"),
        "Error message must mention timeout"
    );
}
