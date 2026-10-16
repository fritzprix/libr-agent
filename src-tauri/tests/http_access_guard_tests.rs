//! Session API Origin/Host access guard (Windows-safe unit coverage).

use tauri_mcp_agent_lib::server::access_guard::{check_access, is_loopback_host};
use warp::http::{HeaderMap, HeaderName, HeaderValue};

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (k, v) in pairs {
        map.insert(
            HeaderName::from_bytes(k.as_bytes()).expect("header name"),
            HeaderValue::from_str(v).expect("header value"),
        );
    }
    map
}

#[test]
fn loopback_hosts_accepted() {
    assert!(is_loopback_host("localhost"));
    assert!(is_loopback_host("localhost:3030"));
    assert!(is_loopback_host("LOCALHOST:3030"));
    assert!(is_loopback_host("127.0.0.1"));
    assert!(is_loopback_host("127.0.0.1:3030"));
    assert!(is_loopback_host("[::1]"));
    assert!(is_loopback_host("[::1]:3030"));
}

#[test]
fn non_loopback_hosts_rejected() {
    assert!(!is_loopback_host("evil.com"));
    assert!(!is_loopback_host("evil.com:3030"));
    assert!(!is_loopback_host("192.168.1.1:3030"));
    assert!(!is_loopback_host("host.docker.internal:3030"));
    assert!(!is_loopback_host(""));
}

#[test]
fn cli_localhost_without_origin_ok() {
    let h = headers(&[("host", "localhost:3030")]);
    assert!(check_access(&h, false).is_ok());
}

#[test]
fn browser_origin_rejected() {
    let h = headers(&[
        ("host", "localhost:3030"),
        ("origin", "https://evil.example"),
    ]);
    assert!(check_access(&h, false).is_err());
}

#[test]
fn non_loopback_host_rejected_when_not_exposed() {
    let h = headers(&[("host", "192.168.1.5:3030")]);
    assert!(check_access(&h, false).is_err());
}

#[test]
fn non_loopback_host_ok_when_exposed() {
    let h = headers(&[("host", "192.168.1.5:3030")]);
    assert!(check_access(&h, true).is_ok());
}

#[test]
fn origin_still_rejected_when_exposed() {
    let h = headers(&[
        ("host", "192.168.1.5:3030"),
        ("origin", "https://evil.example"),
    ]);
    assert!(check_access(&h, true).is_err());
}
