//! Integration tests for shared backend discovery, readiness, and Session API
//! client forwarding (#2701).

use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri_mcp_agent_lib::server::readiness;
use tauri_mcp_agent_lib::server::shared_backend::SharedBackendClient;
use warp::Filter;

#[tokio::test]
async fn shared_backend_waits_for_real_readiness_not_bare_ok() {
    let ready = Arc::new(AtomicBool::new(false));
    let ready_flag = ready.clone();

    let health = warp::path!("api" / "health").and(warp::get()).map(move || {
        let is_ready = ready_flag.load(Ordering::Relaxed);
        warp::reply::json(&json!({
            "status": "ok",
            "service": "libr-agent-session-api",
            "version": "0.9.0",
            "ready": is_ready,
            "recovery": if is_ready { "ok" } else { "pending" }
        }))
    });

    let (addr, server_fut) = warp::serve(health).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port());

    // Bare status="ok" with ready=false must NOT pass readiness wait.
    let err = client
        .wait_until_ready(Duration::from_millis(220))
        .await
        .unwrap_err();
    assert!(err.contains("not ready yet"), "unexpected error: {err}");

    let flip = ready.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(120)).await;
        flip.store(true, Ordering::Relaxed);
    });

    let status = client
        .wait_until_ready(Duration::from_secs(2))
        .await
        .expect("should succeed once ready=true");
    assert!(status.ready);
    assert_eq!(status.recovery, "ok");

    server.abort();
}

#[tokio::test]
async fn shared_backend_client_shares_sessions_and_resumes_on_single_backend() {
    let sessions: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(vec![json!({
        "id": "gui-session-1",
        "name": "GUI Session",
        "status": "idle",
        "model": "gpt-4o",
        "provider": "openai",
        "assistantId": "asst-1",
        "createdAt": 1000,
        "updatedAt": 1000,
        "isBookmarked": false,
        "executionMode": "normal",
        "workspaceIsolation": "host"
    })]));
    let resume_count = Arc::new(AtomicUsize::new(0));

    let list_sessions_store = sessions.clone();
    let list_route = warp::path!("api" / "sessions")
        .and(warp::get())
        .map(move || {
            let items = list_sessions_store.lock().unwrap().clone();
            warp::reply::json(&json!({ "sessions": items, "nextCursor": null }))
        });

    let create_sessions_store = sessions.clone();
    let create_route = warp::path!("api" / "sessions")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |body: serde_json::Value| {
            let id = "tui-session-2".to_string();
            let mut items = create_sessions_store.lock().unwrap();
            items.push(json!({
                "id": id,
                "name": body.get("name").cloned().unwrap_or(json!("TUI Session")),
                "status": "idle",
                "model": "gpt-4o",
                "provider": "openai",
                "assistantId": body.get("assistantId").cloned().unwrap_or(json!("asst-1")),
                "createdAt": 2000,
                "updatedAt": 2000,
                "isBookmarked": false,
                "executionMode": "normal",
                "workspaceIsolation": "host"
            }));
            warp::reply::with_status(
                warp::reply::json(&json!({ "id": "tui-session-2", "status": "idle" })),
                warp::http::StatusCode::CREATED,
            )
        });

    let get_sessions_store = sessions.clone();
    let get_route = warp::path!("api" / "sessions" / String)
        .and(warp::get())
        .map(move |id: String| {
            let items = get_sessions_store.lock().unwrap();
            let found = items
                .iter()
                .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
                .cloned()
                .unwrap_or(json!({ "error": "not found" }));
            warp::reply::json(&found)
        });

    let resume_counter = resume_count.clone();
    let resume_sessions_store = sessions.clone();
    let resume_route = warp::path!("api" / "sessions" / String / "resume")
        .and(warp::post())
        .map(move |id: String| {
            resume_counter.fetch_add(1, Ordering::SeqCst);
            let items = resume_sessions_store.lock().unwrap();
            let found = items
                .iter()
                .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
                .cloned()
                .unwrap_or(json!({ "error": "not found" }));
            warp::reply::json(&found)
        });

    let routes = list_route.or(create_route).or(get_route).or(resume_route);
    let (addr, server_fut) = warp::serve(routes).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let tui_client = SharedBackendClient::new(addr.port());

    // 1. Session started in GUI is visible and resumable from TUI.
    let listed = tui_client.list_sessions(None).await.unwrap();
    assert_eq!(listed["sessions"][0]["id"], "gui-session-1");
    let resumed = tui_client.resume_session("gui-session-1").await.unwrap();
    assert_eq!(resumed["id"], "gui-session-1");
    assert_eq!(resume_count.load(Ordering::SeqCst), 1);

    // 2. Session created from TUI is visible in the shared store for GUI.
    let created = tui_client
        .create_session(&json!({ "name": "TUI Session", "assistantId": "asst-1" }))
        .await
        .unwrap();
    assert_eq!(created["id"], "tui-session-2");

    let listed_after = tui_client.list_sessions(None).await.unwrap();
    assert_eq!(listed_after["sessions"].as_array().unwrap().len(), 2);

    // Verify readiness state enum still exposes Skipped for attached frontends.
    let _ = readiness::RecoveryState::Skipped;

    server.abort();
}
