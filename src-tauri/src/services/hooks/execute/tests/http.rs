use super::*;

/// Serves exactly one HTTP request, returning `response_body` after
/// checking the request line and headers contain `assertions` substrings.
/// Returns the accepted request body.
async fn serve_one_http_request(
    listener: tokio::net::TcpListener,
    response_body: &'static str,
    assertions: Vec<&'static str>,
) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut conn, _) = listener.accept().await.expect("accept");
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let n = conn.read(&mut chunk).await.expect("read request");
        if n == 0 {
            panic!("request ended before headers were complete");
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
    }
    let headers = String::from_utf8_lossy(&buf[..header_end]).to_lowercase();
    let content_length: usize = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    while buf.len() < header_end + content_length {
        let n = conn.read(&mut chunk).await.expect("read body");
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&buf[header_end..]).to_string();
    let haystack = format!(
        "{}\n{body}",
        String::from_utf8_lossy(&buf[..header_end]).to_lowercase()
    );
    for needle in assertions {
        assert!(
            haystack.contains(needle),
            "request should contain {needle:?}, got: {haystack}"
        );
    }
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response_body}",
        response_body.len()
    );
    conn.write_all(response.as_bytes())
        .await
        .expect("write response");
    body
}

/// #3009: a `type: "http"` handler POSTs the event envelope and the
/// 2xx JSON body is applied with the same decision fields as command
/// hooks. Also checks header interpolation follows `allowedEnvVars`.
#[tokio::test]
async fn http_handler_posts_envelope_and_applies_json_decision() {
    std::env::set_var("TEST_HOOK_TOKEN", "tok-42");
    let json_out: &'static str = r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"http says no"}}"#;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let url = format!("http://{}/pre-tool-use", listener.local_addr().unwrap());
    let server = tokio::spawn(serve_one_http_request(
        listener,
        json_out,
        vec![
            "content-type: application/json",
            "authorization: bearer tok-42",
            "x-unlisted: ",
            "hook_event_name",
        ],
    ));

    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [{
                    "type": "http",
                    "url": url,
                    "headers": {
                        "Authorization": "Bearer $TEST_HOOK_TOKEN",
                        "X-Unlisted": "[$NOT_LISTED]"
                    },
                    "allowedEnvVars": ["TEST_HOOK_TOKEN"]
                }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("http-hook-{}", uuid::Uuid::new_v4());
    let mut params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    params.tool_name = Some("workspace__runShell".to_string());
    let outcome = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    assert_eq!(
        outcome.decision,
        Some(HookDecision::Block),
        "http response decision fields must apply"
    );
    assert_eq!(outcome.reason.as_deref(), Some("http says no"));

    let body = tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .expect("server task")
        .expect("server task join");
    assert!(
        body.contains("workspace__runShell"),
        "POST body is the event envelope: {body}"
    );
    std::env::remove_var("TEST_HOOK_TOKEN");
    clear_hook_context(&sid);
}

/// #3009: a non-2xx response is a non-blocking error; the action goes
/// ahead with no decision from the hook.
#[tokio::test]
async fn http_handler_non_2xx_is_non_blocking() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut conn, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 1024];
        let _ = conn.read(&mut buf).await;
        let response =
            "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
        conn.write_all(response.as_bytes()).await.expect("write");
    });

    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [{ "type": "http", "url": format!("http://{addr}/fail") }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("http-hook-500-{}", uuid::Uuid::new_v4());
    let params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    let outcome = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    assert_eq!(outcome.decision, None, "non-2xx must not block");

    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server).await;
    clear_hook_context(&sid);
}

/// #3041 G1.2: redirects must never be followed — a 3xx would re-POST
/// the envelope and custom headers to a host the user never approved.
/// The redirect target must receive nothing, and the 3xx itself is a
/// non-blocking error.
#[tokio::test]
async fn http_handler_does_not_follow_redirects() {
    let redirect_target = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind redirect target");
    let target_addr = redirect_target.local_addr().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let (mut conn, _) = listener.accept().await.expect("accept");
        let response = format!(
            "HTTP/1.1 302 Found\r\nlocation: http://{target_addr}/stolen\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        );
        conn.write_all(response.as_bytes()).await.expect("write");
        drop(conn);
        // The client must not connect to the redirect target.
        tokio::time::timeout(std::time::Duration::from_secs(1), redirect_target.accept())
            .await
            .is_err()
    });

    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [{ "type": "http", "url": format!("http://{addr}/redirect") }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("http-hook-redirect-{}", uuid::Uuid::new_v4());
    let params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    let outcome = dispatch_hook_event_in(
        temp.path(),
        HookEvent::PreToolUse,
        "workspace__runShell",
        &params,
        &sid,
    )
    .await;
    assert_eq!(outcome.decision, None, "3xx must not block");
    // server returns true when the redirect target accepted nothing.
    assert!(
        server.await.expect("server task"),
        "redirect target must receive no connection"
    );
    clear_hook_context(&sid);
}

/// #3041 G1.3: one deadline covers send AND body read — a server that
/// accepts and then stalls must not block the tool call past the
/// handler timeout.
#[tokio::test]
async fn http_handler_stalled_body_times_out() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut conn, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 1024];
        let _ = conn.read(&mut buf).await;
        // Send the headers, then stall before any body byte: the
        // original failure mode was `response.text()` waiting forever
        // after `send()` had already succeeded (#3041 S2.2).
        let _ = conn
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\n")
            .await;
        // Outlive the handler timeout so the client must give up on
        // its own.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        drop(conn);
    });

    let hooks_json = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "workspace__runShell",
                "hooks": [{ "type": "http", "url": format!("http://{addr}/stall"), "timeout": 1 }]
            }]
        }
    })
    .to_string();
    let (temp, root) = setup_approved_plugin(&hooks_json);

    let sid = format!("http-hook-stall-{}", uuid::Uuid::new_v4());
    let params = HookEnvelopeParams::new(&sid, root.clone(), HookEvent::PreToolUse);
    let started = std::time::Instant::now();
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
        started.elapsed() < std::time::Duration::from_secs(4),
        "deadline must cover the body read, not just send"
    );
    let _ = server;
    clear_hook_context(&sid);
}
