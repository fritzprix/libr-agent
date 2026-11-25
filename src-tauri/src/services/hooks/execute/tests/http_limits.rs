use super::super::http::interpolate_http_header;
use super::*;

/// #3041 G2.1: a valid deny decision padded with `reason_bytes` of `a`
/// to reach the requested size. Removing the size bound would let this
/// decision apply, so every oversized-body test below fails without the
/// bound — unlike a non-JSON body, which is skipped anyway.
fn padded_deny_json(reason_bytes: usize) -> String {
    format!(
        r#"{{"hookSpecificOutput":{{"permissionDecision":"deny","permissionDecisionReason":"{}"}}}}"#,
        "a".repeat(reason_bytes)
    )
}

/// #3041 G2.1: dispatch one PreToolUse against a loopback server that
/// replies 200 with `body`, served with (`with_length`) or without a
/// `content-length` header — without it, the chunked read loop is the
/// only thing enforcing the bound.
async fn dispatch_body_size(body: String, with_length: bool) -> HookDispatchResult {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut conn, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 4096];
        let _ = conn.read(&mut buf).await;
        let headers = if with_length {
            format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            )
        } else {
            "HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n".to_string()
        };
        let _ = conn.write_all(headers.as_bytes()).await;
        // The client stops reading once the bound is crossed, so these
        // writes may fail with a broken pipe; that is expected.
        let _ = conn.write_all(body.as_bytes()).await;
        let _ = conn.flush().await;
        drop(conn);
    });

    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [{ "type": "http", "url": format!("http://{addr}/body-size") }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("http-hook-size-{}", uuid::Uuid::new_v4());
    let params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    let outcome = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
    clear_hook_context(&sid);
    outcome
}

/// #3041 G2.1: with `content-length`, a valid deny decision padded past
/// the bound must be rejected by the header pre-check, not applied.
#[tokio::test]
async fn http_handler_rejects_oversized_body_with_content_length() {
    let outcome = dispatch_body_size(padded_deny_json(2 * 1024 * 1024), true).await;
    assert_eq!(
        outcome.decision, None,
        "oversized deny decision must not be applied"
    );
}

/// #3041 G2.1: without `content-length`, the same oversized decision
/// must be rejected by the chunked read loop, not applied.
#[tokio::test]
async fn http_handler_rejects_oversized_body_chunked() {
    let outcome = dispatch_body_size(padded_deny_json(2 * 1024 * 1024), false).await;
    assert_eq!(
        outcome.decision, None,
        "oversized streamed deny decision must not be applied"
    );
}

/// #3041 G2.1 control: the same decision shape under the bound is
/// applied, proving the oversized tests fail because of the size, not
/// because http decisions never work.
#[tokio::test]
async fn http_handler_applies_decision_under_body_limit() {
    let outcome = dispatch_body_size(padded_deny_json(64 * 1024), true).await;
    assert_eq!(
        outcome.decision,
        Some(HookDecision::Block),
        "under-limit deny decision must apply"
    );
    assert!(outcome
        .reason
        .as_deref()
        .is_some_and(|reason| reason.starts_with('a')));
}

/// #3041 G1.5: an invalid header value must skip the hook with a
/// warning — never send a mangled request the user did not approve.
#[tokio::test]
async fn http_handler_invalid_header_value_skips_hook() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        // The hook must skip before connecting: no accept within 1s.
        tokio::time::timeout(std::time::Duration::from_secs(1), listener.accept())
            .await
            .is_err()
    });

    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [{
                    "type": "http",
                    "url": format!("http://{addr}/bad-header"),
                    "headers": { "X-Bad": "line\nbreak" }
                }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("http-hook-badhdr-{}", uuid::Uuid::new_v4());
    let params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    let outcome = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    assert_eq!(outcome.decision, None);
    assert!(
        server.await.expect("server task"),
        "hook must skip before connecting"
    );
    clear_hook_context(&sid);
}

/// #3041 G1.4: two handlers on the same event, matcher, and URL that
/// differ only by headers must each get dispatched — the approval key
/// has to include everything that distinguishes them.
#[tokio::test]
async fn two_http_handlers_differing_only_by_headers_each_dispatch() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut seen = String::new();
        for _ in 0..2 {
            let (mut conn, _) = listener.accept().await.expect("accept");
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = conn.read(&mut chunk).await.expect("read");
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            seen.push_str(&String::from_utf8_lossy(&buf).to_lowercase());
            let response = "HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
            conn.write_all(response.as_bytes()).await.expect("write");
        }
        seen
    });

    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [
                    { "type": "http", "url": format!("http://{addr}/dual"), "headers": { "X-One": "1" } },
                    { "type": "http", "url": format!("http://{addr}/dual"), "headers": { "X-Two": "2" } }
                ]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("http-hook-dual-{}", uuid::Uuid::new_v4());
    let mut params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    params.tool_name = Some("workspace__runShell".to_string());
    let _outcome = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    let seen = tokio::time::timeout(std::time::Duration::from_secs(10), server)
        .await
        .expect("both requests")
        .expect("server join");
    assert!(seen.contains("x-one: 1"), "first handler must run: {seen}");
    assert!(seen.contains("x-two: 2"), "second handler must run: {seen}");
    clear_hook_context(&sid);
}

#[test]
fn http_header_interpolation_follows_allowlist() {
    std::env::set_var("TEST_INTERP_A", "value-a");
    let allowed: Vec<String> = vec![
        "TEST_INTERP_A".to_string(),
        "TEST_INTERP_MISSING".to_string(),
    ];
    assert_eq!(
        interpolate_http_header("Bearer ${TEST_INTERP_A} and $TEST_INTERP_A", &allowed),
        "Bearer value-a and value-a"
    );
    assert_eq!(
        interpolate_http_header("[$TEST_INTERP_MISSING]", &allowed),
        "[]",
        "allowlisted but unset resolves to empty"
    );
    assert_eq!(
        interpolate_http_header("[$NOT_ALLOWED]", &allowed),
        "[]",
        "unlisted references become empty"
    );
    assert_eq!(
        interpolate_http_header("$TEST_INTERP_A", &[]),
        "$TEST_INTERP_A",
        "no allowlist means no interpolation"
    );
    std::env::remove_var("TEST_INTERP_A");
}
