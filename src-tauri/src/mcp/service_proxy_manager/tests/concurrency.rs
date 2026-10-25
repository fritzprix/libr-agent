use super::*;

/// Concurrent create_proxy for the same session must single-flight to one Arc.
#[tokio::test]
async fn test_concurrent_create_proxy_same_session_single_flight() {
    let harness = create_test_harness().await;
    let manager = Arc::clone(&harness.manager);
    let session_id = "concurrent-create-same-session";

    insert_test_session(&manager, session_id).await;

    let manager_a = Arc::clone(&manager);
    let manager_b = Arc::clone(&manager);
    let session_a = session_id.to_string();
    let session_b = session_id.to_string();

    let (result_a, result_b) = tokio::join!(
        manager_a.create_proxy(session_a, vec!["playbook".to_string()], vec![], None),
        manager_b.create_proxy(session_b, vec!["playbook".to_string()], vec![], None),
    );

    let proxy_a = result_a.expect("first create_proxy must succeed");
    let proxy_b = result_b.expect("second create_proxy must succeed");
    assert!(
        Arc::ptr_eq(&proxy_a, &proxy_b),
        "concurrent create_proxy must return the same proxy Arc"
    );
    assert!(
        Arc::ptr_eq(
            &proxy_a,
            &manager.get_proxy(session_id).await.expect("proxy present")
        ),
        "map must hold the single published proxy"
    );
}

/// ensure_builtin_proxy and create_proxy on the same session must serialize;
/// final proxy must be the full (non-builtin-only) create path.
#[tokio::test]
async fn test_ensure_builtin_and_create_proxy_serialize() {
    let harness = create_test_harness().await;
    let manager = Arc::clone(&harness.manager);
    let session_id = "ensure-builtin-create-serialize";

    insert_test_session(&manager, session_id).await;

    let manager_a = Arc::clone(&manager);
    let manager_b = Arc::clone(&manager);
    let session_owned = session_id.to_string();

    let (ensure_result, create_result) = tokio::join!(
        manager_a.ensure_builtin_proxy(session_id),
        manager_b.create_proxy(session_owned, vec!["playbook".to_string()], vec![], None),
    );

    let ensure_proxy = ensure_result.expect("ensure_builtin_proxy must succeed");
    let create_proxy = create_result.expect("create_proxy must succeed");
    let final_proxy = manager
        .get_proxy(session_id)
        .await
        .expect("proxy must exist after both settle");

    assert!(
        !final_proxy.is_builtin_only(),
        "final proxy must match full create_proxy upgrade path"
    );
    assert!(
        Arc::ptr_eq(&final_proxy, &create_proxy),
        "map proxy must be the create_proxy result after serialization"
    );
    // ensure may have observed the pre-upgrade lazy proxy or the final one.
    assert!(
        Arc::ptr_eq(&ensure_proxy, &final_proxy) || ensure_proxy.is_builtin_only(),
        "ensure_builtin must return either the final proxy or a prior builtin-only Arc"
    );
}

/// Overlapping create_proxy and destroy_proxy must settle without panic or dangling managers.
#[tokio::test]
async fn test_create_proxy_overlapping_destroy_proxy() {
    let harness = create_test_harness().await;
    let manager = Arc::clone(&harness.manager);
    let session_id = "create-destroy-overlap";

    insert_test_session(&manager, session_id).await;

    // Warm path: establish a proxy first so destroy has work, then overlap recreate+destroy.
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .expect("initial create must succeed");

    let manager_create = Arc::clone(&manager);
    let manager_destroy = Arc::clone(&manager);
    let session_owned = session_id.to_string();

    let (create_result, ()) = tokio::join!(
        manager_create.create_proxy(session_owned, vec!["playbook".to_string()], vec![], None),
        manager_destroy.destroy_proxy(session_id),
    );

    // create may Ok (ran after destroy) or Ok (ran before / reused then destroyed).
    let _ = create_result;

    let proxy = manager.get_proxy(session_id).await;
    let has_stdio = manager.has_stdio_manager_for_test(session_id).await;
    let has_http = manager.has_http_manager_for_test(session_id).await;
    let has_guard = manager.has_creation_guard_for_test(session_id).await;

    match proxy {
        None => {
            assert!(
                !has_stdio && !has_http,
                "destroyed session must not leave dangling managers"
            );
            assert!(
                !has_guard,
                "destroy must remove creation guard when no proxy remains"
            );
        }
        Some(p) => {
            // Only valid if create started after destroy finished and re-published.
            assert!(
                has_stdio && has_http,
                "live proxy must have matching managers"
            );
            assert!(
                !p.is_builtin_only(),
                "post-destroy recreate must be a full proxy"
            );
            assert!(
                has_guard,
                "active session after recreate may retain creation guard"
            );
        }
    }

    // Second destroy must clean any surviving create-after-destroy proxy.
    manager.destroy_proxy(session_id).await;
    assert!(manager.get_proxy(session_id).await.is_none());
    assert!(!manager.has_stdio_manager_for_test(session_id).await);
    assert!(!manager.has_http_manager_for_test(session_id).await);
    assert!(!manager.has_creation_guard_for_test(session_id).await);
}
