//! Integration tests for [`SharedBackendClient`] (#2908).
//!
//! Exercises readiness polling (`wait_until_ready`) and Session API calls
//! (`list_sessions`, `list_assistants`) against ephemeral loopback fake HTTP
//! servers.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri_mcp_agent_lib::server::shared_backend::SharedBackendClient;
use warp::http::{Response, StatusCode};
use warp::Filter;

fn health_json(ready: bool, recovery: &str) -> Response<String> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(
            json!({
                "status": "ok",
                "service": "libr-agent-session-api",
                "version": "0.9.31",
                "ready": ready,
                "recovery": recovery,
            })
            .to_string(),
        )
        .unwrap()
}

fn raw_response(status: StatusCode, body: &str) -> Response<String> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body.to_string())
        .unwrap()
}

#[tokio::test]
async fn wait_until_ready_returns_immediately_when_ready() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_route = calls.clone();

    let route = warp::path!("api" / "health").and(warp::get()).map(move || {
        calls_route.fetch_add(1, Ordering::SeqCst);
        health_json(true, "ok")
    });

    let (addr, server_fut) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();
    let status = client
        .wait_until_ready(Duration::from_secs(2))
        .await
        .expect("immediately ready backend should succeed");

    assert!(status.ready);
    assert_eq!(status.status, "ok");
    assert_eq!(status.recovery, "ok");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    server.abort();
}

#[tokio::test]
async fn wait_until_ready_times_out_when_never_ready() {
    let route = warp::path!("api" / "health")
        .and(warp::get())
        .map(|| health_json(false, "pending"));

    let (addr, server_fut) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();
    let err = client
        .wait_until_ready(Duration::from_millis(220))
        .await
        .expect_err("never-ready backend must time out");

    assert!(err.contains("Timed out"), "unexpected error: {err}");
    assert!(err.contains("not ready yet"), "unexpected error: {err}");

    server.abort();
}

#[tokio::test]
async fn wait_until_ready_succeeds_on_late_ready() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_route = calls.clone();

    let route = warp::path!("api" / "health").and(warp::get()).map(move || {
        let attempt = calls_route.fetch_add(1, Ordering::SeqCst);
        match attempt {
            0 => raw_response(StatusCode::SERVICE_UNAVAILABLE, "starting"),
            1 => health_json(false, "pending"),
            _ => health_json(true, "ok"),
        }
    });

    let (addr, server_fut) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();
    let status = client
        .wait_until_ready(Duration::from_secs(2))
        .await
        .expect("late-ready backend should succeed");

    assert!(status.ready);
    assert_eq!(status.recovery, "ok");
    assert_eq!(calls.load(Ordering::SeqCst), 3);

    server.abort();
}

#[tokio::test]
async fn wait_until_ready_fails_fast_on_unauthorized_and_times_out_on_bad_body() {
    let unauth_calls = Arc::new(AtomicUsize::new(0));
    let unauth_calls_route = unauth_calls.clone();
    let unauth_route = warp::path!("api" / "health").and(warp::get()).map(move || {
        unauth_calls_route.fetch_add(1, Ordering::SeqCst);
        raw_response(StatusCode::UNAUTHORIZED, r#"{"error":"unauthorized"}"#)
    });
    let (unauth_addr, unauth_fut) = warp::serve(unauth_route).bind_ephemeral(([127, 0, 0, 1], 0));
    let unauth_server = tokio::spawn(unauth_fut);

    let client = SharedBackendClient::new(unauth_addr.port()).unwrap();
    let started = Instant::now();
    let err = client
        .wait_until_ready(Duration::from_secs(3))
        .await
        .expect_err("401 must fail immediately");
    assert!(err.contains("401 Unauthorized"), "unexpected error: {err}");
    assert_eq!(unauth_calls.load(Ordering::SeqCst), 1);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "401 should not wait for timeout; elapsed={:?}",
        started.elapsed()
    );
    unauth_server.abort();

    let bad_body_route = warp::path!("api" / "health")
        .and(warp::get())
        .map(|| raw_response(StatusCode::OK, "not-valid-json"));
    let (bad_addr, bad_fut) = warp::serve(bad_body_route).bind_ephemeral(([127, 0, 0, 1], 0));
    let bad_server = tokio::spawn(bad_fut);

    let bad_client = SharedBackendClient::new(bad_addr.port()).unwrap();
    let bad_err = bad_client
        .wait_until_ready(Duration::from_millis(180))
        .await
        .expect_err("malformed health JSON must fail");
    assert!(
        bad_err.contains("Invalid /api/health response"),
        "unexpected error: {bad_err}"
    );
    bad_server.abort();
}

#[tokio::test]
async fn list_sessions_and_list_assistants_return_parsed_json_and_forward_limit() {
    let sessions_route = warp::path!("api" / "sessions")
        .and(warp::get())
        .and(warp::query::<HashMap<String, String>>())
        .map(|params: HashMap<String, String>| {
            let limit = params.get("limit").cloned();
            warp::reply::json(&json!({
                "sessions": [{ "id": "s-1" }],
                "observedLimit": limit,
            }))
        });

    let assistants_route = warp::path!("api" / "assistants").and(warp::get()).map(|| {
        warp::reply::json(&json!({
            "assistants": [{ "id": "a-1", "name": "Helper" }],
        }))
    });

    let routes = sessions_route.or(assistants_route);
    let (addr, server_fut) = warp::serve(routes).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();

    let all_sessions = client
        .list_sessions(None)
        .await
        .expect("list_sessions(None) should succeed");
    assert_eq!(all_sessions["sessions"][0]["id"], "s-1");
    assert_eq!(all_sessions["observedLimit"], Value::Null);

    let limited_sessions = client
        .list_sessions(Some(15))
        .await
        .expect("list_sessions(Some(15)) should succeed");
    assert_eq!(limited_sessions["observedLimit"], "15");

    let assistants = client
        .list_assistants()
        .await
        .expect("list_assistants should succeed");
    assert_eq!(assistants["assistants"][0]["id"], "a-1");

    server.abort();
}

#[tokio::test]
async fn list_sessions_maps_http_errors_and_bad_bodies_to_error_strings() {
    let step = Arc::new(AtomicUsize::new(0));
    let step_route = step.clone();

    let route = warp::path!("api" / "sessions")
        .and(warp::get())
        .map(move || match step_route.fetch_add(1, Ordering::SeqCst) {
            0 => raw_response(StatusCode::UNAUTHORIZED, r#"{"error":"bad key"}"#),
            1 => raw_response(StatusCode::NOT_FOUND, ""),
            2 => raw_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                r#"{"error":"session store locked"}"#,
            ),
            3 => raw_response(StatusCode::INTERNAL_SERVER_ERROR, "upstream exploded"),
            _ => raw_response(StatusCode::OK, "{broken json"),
        });

    let (addr, server_fut) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();

    let err_401 = client.list_sessions(None).await.unwrap_err();
    assert!(
        err_401.contains("401 Unauthorized"),
        "unexpected 401 error: {err_401}"
    );

    let err_404 = client.list_sessions(None).await.unwrap_err();
    assert!(
        err_404.contains("404 Not Found"),
        "unexpected 404 error: {err_404}"
    );

    let err_500_json = client.list_sessions(None).await.unwrap_err();
    assert_eq!(err_500_json, "session store locked");

    let err_500_plain = client.list_sessions(None).await.unwrap_err();
    assert!(
        err_500_plain.contains("500") && err_500_plain.contains("upstream exploded"),
        "unexpected 500 plain error: {err_500_plain}"
    );

    let err_bad_json = client.list_sessions(Some(5)).await.unwrap_err();
    assert!(
        err_bad_json.contains("Invalid JSON"),
        "unexpected bad-body error: {err_bad_json}"
    );

    server.abort();
}

#[tokio::test]
async fn list_assistants_maps_http_errors_and_bad_bodies_to_error_strings() {
    let step = Arc::new(AtomicUsize::new(0));
    let step_route = step.clone();

    let route = warp::path!("api" / "assistants")
        .and(warp::get())
        .map(move || match step_route.fetch_add(1, Ordering::SeqCst) {
            0 => raw_response(StatusCode::UNAUTHORIZED, ""),
            1 => raw_response(StatusCode::NOT_FOUND, ""),
            2 => raw_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                r#"{"error":"assistant repo unavailable"}"#,
            ),
            3 => raw_response(StatusCode::BAD_GATEWAY, "gateway timeout text"),
            _ => raw_response(StatusCode::OK, "not-json-at-all"),
        });

    let (addr, server_fut) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();

    let err_401 = client.list_assistants().await.unwrap_err();
    assert!(
        err_401.contains("401 Unauthorized"),
        "unexpected 401 error: {err_401}"
    );

    let err_404 = client.list_assistants().await.unwrap_err();
    assert!(
        err_404.contains("404 Not Found"),
        "unexpected 404 error: {err_404}"
    );

    let err_500_json = client.list_assistants().await.unwrap_err();
    assert_eq!(err_500_json, "assistant repo unavailable");

    let err_502_plain = client.list_assistants().await.unwrap_err();
    assert!(
        err_502_plain.contains("502") && err_502_plain.contains("gateway timeout text"),
        "unexpected 502 plain error: {err_502_plain}"
    );

    let err_bad_json = client.list_assistants().await.unwrap_err();
    assert!(
        err_bad_json.contains("Invalid JSON"),
        "unexpected bad-body error: {err_bad_json}"
    );

    server.abort();
}
