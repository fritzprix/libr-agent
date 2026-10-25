use super::*;

/// Verify that lazy builtin proxies set `is_builtin_only() == true`
/// and full proxies set `is_builtin_only() == false`.
#[tokio::test]
async fn test_builtin_only_proxy_flag_and_upgrade() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "builtin-only-flag-test";

    insert_test_session(&manager, session_id).await;

    // 1. Lazy builtin proxy must have is_builtin_only == true
    let lazy_proxy = manager.ensure_builtin_proxy(session_id).await.unwrap();
    assert!(
        lazy_proxy.is_builtin_only(),
        "Lazy builtin proxy must be marked as is_builtin_only"
    );

    // 2. Fully configured proxy must have is_builtin_only == false
    let configured_proxy = manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();
    assert!(
        !configured_proxy.is_builtin_only(),
        "Fully configured proxy must NOT be marked as is_builtin_only"
    );
}

#[test]
fn test_promote_builtin_reuse_runtime_state() {
    use super::promote_builtin_reuse_runtime_state;
    use crate::agent::runtime_state::SessionRuntimeState;

    let not_ready = SessionRuntimeState::default();
    assert!(!not_ready.proxy.ready);
    let promoted = promote_builtin_reuse_runtime_state(not_ready, false);
    assert!(promoted.proxy.ready);

    let ready = SessionRuntimeState::builtin_ready();
    let kept = promote_builtin_reuse_runtime_state(ready.clone(), false);
    assert_eq!(kept, ready);

    let external_unready = SessionRuntimeState::default();
    let unchanged = promote_builtin_reuse_runtime_state(external_unready.clone(), true);
    assert_eq!(unchanged, external_unready);
}

/// Reuse after ensure_builtin must promote a non-ready store back to ready so
/// open()/FE can leave Hydrating when the lazy path wrote ready without emit.
#[tokio::test]
async fn test_reuse_after_lazy_builtin_promotes_unready_runtime_state() {
    use crate::agent::runtime_state::SessionRuntimeState;

    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "reuse-promote-unready-runtime";

    insert_test_session(manager, session_id).await;

    let lazy_proxy = manager.ensure_builtin_proxy(session_id).await.unwrap();
    let tool_ids = lazy_proxy.builtin_tool_ids();
    assert!(
        manager.get_runtime_state(session_id).await.proxy.ready,
        "ensure_builtin_proxy must leave ready=true"
    );

    // Simulate lost/incomplete runtime snapshot while the proxy Arc remains.
    manager
        .set_runtime_state(session_id, SessionRuntimeState::default(), None)
        .await;
    assert!(!manager.get_runtime_state(session_id).await.proxy.ready);

    let reused = manager
        .create_proxy(session_id.to_string(), tool_ids, vec![], None)
        .await
        .unwrap();
    assert!(
        Arc::ptr_eq(&lazy_proxy, &reused),
        "matching builtin-only create_proxy must Reuse the lazy proxy"
    );
    assert!(
        manager.get_runtime_state(session_id).await.proxy.ready,
        "Reuse must promote builtin-only sessions to ready"
    );
}
