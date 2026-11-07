use super::*;

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
/// bootstrap, workflow start, and reload) re-enables tool calls.
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

    // A new lifecycle entry (session create / workflow start / reload) clears
    // the fence, and the session can work again.
    manager.clear_teardown_fence(session_id).await;
    manager
        .call_tool(session_id, "playbook__listPlaybooks", json!({}))
        .await
        .expect("call_tool must work again after the fence is cleared");
}

/// Idle/cancel paths must leave the fence untouched: only explicit terminate
/// fences, so ordinary pollers and soft cancels never poison a live session.
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
