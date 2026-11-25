#![cfg(not(windows))]
//! Real-listener coverage for the loopback peer guard (#2956).
//!
//! warp's `serve_incoming` wrapped every accepted socket in an internal
//! `LiftIo` whose `remote_addr()` is `None`, so the E2E control plane and the
//! pairing route answered 403 "pairing is only permitted from loopback" even
//! for genuine `127.0.0.1` requests. `warp::test` sets the route-level
//! address by hand, so the filter tests could not catch this; the guard must
//! be exercised through the real accept path (`server::serve_http`, which
//! `server::init` itself calls) with real TCP sockets instead.
//!
//! The full `server::init` cannot run in a headless test: it needs an owned
//! `tauri::AppHandle` on the real Wry runtime, which only a started desktop
//! app has. The accept path, the peer-extension tag, and the production
//! guard chain are all exercised here; `routes.rs` wiring for the same chain
//! is pinned by `e2e_control_token_tests`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use tauri_mcp_agent_lib::agent::AgentSessionManager;
use tauri_mcp_agent_lib::server::access_guard::handle_rejection;
use tauri_mcp_agent_lib::server::e2e_control::e2e_control_guarded_routes_with_capability;
use tauri_mcp_agent_lib::server::serve_http;
use warp::Filter;

const TOKEN: &str = "test-capability-token-0123456789abcdef";

/// Manager filter that must never be extracted: the status route answers
/// before the manager is needed, so reaching it means a handler ran that
/// this test never wired up.
fn forbidden_manager(
) -> impl Filter<Extract = (Arc<AgentSessionManager>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(|| -> Arc<AgentSessionManager> {
        panic!("agent manager extracted although this route must not need it")
    })
}

fn http_get(port: u16, path: &str, capability: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to server");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("read timeout");
    let mut request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    if let Some(capability) = capability {
        request.push_str(&format!(
            "{}: {capability}\r\n",
            tauri_mcp_agent_lib::server::e2e_control::E2E_CONTROL_HEADER
        ));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).expect("write request");
    let mut response = Vec::new();
    // `Connection: close` makes read_to_end terminate once the reply is sent.
    let _ = stream.read_to_end(&mut response);
    let text = String::from_utf8_lossy(&response).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    (status, text)
}

// `serve_http` serves on a spawned task; the client here uses blocking
// sockets, so the test needs a worker thread to run the server task while
// the test thread waits on the reply.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_loopback_socket_passes_the_peer_guard() {
    // The exact production guard chain `server::routes` mounts for the E2E
    // control plane: Host/Origin guard, loopback peer guard, capability guard.
    let routes =
        e2e_control_guarded_routes_with_capability(Some(TOKEN.to_string()), forbidden_manager())
            .recover(handle_rejection);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind listener");
    let port = listener.local_addr().expect("local_addr").port();
    let _server = serve_http(listener, routes);

    // 1. A real 127.0.0.1 socket with the capability must be served. Before
    //    #2956 this answered 403 "pairing is only permitted from loopback"
    //    because the accepted socket's peer address never reached the guard.
    let (status, body) = http_get(port, "/api/e2e-control/status", Some(TOKEN));
    assert_eq!(
        status, 200,
        "loopback + capability must be served; got: {body}"
    );
    assert!(
        body.contains("\"enabled\":true"),
        "status body must report the enabled control plane; got: {body}"
    );

    // 2. The peer-address fix must not widen the guard: without the
    //    capability header the same guarded route stays 403.
    let (status, body) = http_get(port, "/api/e2e-control/status", None);
    assert_eq!(
        status, 403,
        "missing capability must stay rejected; got: {body}"
    );

    // 3. A wrong capability is rejected the same way.
    let (status, body) = http_get(
        port,
        "/api/e2e-control/status",
        Some("definitely-not-the-token"),
    );
    assert_eq!(
        status, 403,
        "wrong capability must stay rejected; got: {body}"
    );
}
