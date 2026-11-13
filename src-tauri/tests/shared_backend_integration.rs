//! Integration tests for shared backend discovery, readiness, auth guard,
//! attach-mode rejection, and Session API proxy mapping (#2701).

use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri_mcp_agent_lib::commands::agent_commands::shared_proxy;
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tauri_mcp_agent_lib::server::handlers::types::{ListSessionsResponse, SessionStatusResponse};
use tauri_mcp_agent_lib::server::shared_backend::SharedBackendClient;
use warp::Filter;

/// Fixture built from the real [`SessionMetadata`] type so the mock server
/// replies serialize exactly what `server/routes.rs` sends to the frontend.
fn gui_session_metadata() -> SessionMetadata {
    SessionMetadata {
        id: "gui-session-1".to_string(),
        name: Some("GUI Session".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        assistant_id: Some("asst-1".to_string()),
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1000,
        updated_at: 1000,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
    }
}

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

    let client = SharedBackendClient::new(addr.port()).unwrap();

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
async fn wait_until_ready_fails_fast_on_401_health() {
    let health = warp::path!("api" / "health").and(warp::get()).map(|| {
        warp::reply::with_status(
            warp::reply::json(&json!({ "error": "unauthorized" })),
            warp::http::StatusCode::UNAUTHORIZED,
        )
    });

    let (addr, server_fut) = warp::serve(health).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();
    let started = Instant::now();
    let err = client
        .wait_until_ready(Duration::from_secs(30))
        .await
        .unwrap_err();
    let elapsed = started.elapsed();

    assert!(err.contains("401 Unauthorized"), "unexpected error: {err}");
    // A 401 is permanent for a keyless client: the wait must not burn the
    // whole timeout retrying it.
    assert!(
        elapsed < Duration::from_secs(5),
        "wait_until_ready kept polling a 401: {elapsed:?}"
    );

    server.abort();
}

#[tokio::test]
async fn shared_backend_client_surfaces_explicit_401() {
    let route = warp::path!("api" / "sessions").and(warp::get()).map(|| {
        warp::reply::with_status(
            warp::reply::json(&json!({ "error": "unauthorized" })),
            warp::http::StatusCode::UNAUTHORIZED,
        )
    });

    let (addr, server_fut) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();
    let err = client.list_sessions(None).await.unwrap_err();
    assert!(err.contains("401 Unauthorized"), "unexpected error: {err}");

    server.abort();
}

#[tokio::test]
async fn shared_proxy_maps_real_shapes_and_surfaces_errors() {
    let sessions: Arc<Mutex<Vec<SessionMetadata>>> =
        Arc::new(Mutex::new(vec![gui_session_metadata()]));
    let resume_count = Arc::new(AtomicUsize::new(0));

    // Mock replies are built from the real ListSessionsResponse /
    // SessionStatusResponse types, so a field rename in handlers/types.rs
    // fails these tests instead of drifting silently.
    let list_store = sessions.clone();
    let list_route = warp::path!("api" / "sessions")
        .and(warp::get())
        .map(move || {
            let sessions = list_store.lock().unwrap().clone();
            let body = serde_json::to_value(ListSessionsResponse {
                sessions,
                next_cursor: None,
            })
            .unwrap();
            warp::reply::json(&body)
        });

    let get_store = sessions.clone();
    let get_route = warp::path!("api" / "sessions" / String)
        .and(warp::get())
        .map(move |id: String| {
            if id == "server-error" {
                return warp::reply::with_status(
                    warp::reply::json(&json!({ "error": "boom" })),
                    warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                );
            }
            let items = get_store.lock().unwrap();
            if let Some(found) = items.iter().find(|s| s.id == id) {
                let body = serde_json::to_value(SessionStatusResponse {
                    session: found.clone(),
                    pending_approvals: Vec::new(),
                    pending_user_prompt: None,
                })
                .unwrap();
                warp::reply::with_status(warp::reply::json(&body), warp::http::StatusCode::OK)
            } else {
                warp::reply::with_status(
                    warp::reply::json(&json!({ "error": "not found" })),
                    warp::http::StatusCode::NOT_FOUND,
                )
            }
        });

    let msgs_route = warp::path!("api" / "sessions" / String / "messages")
        .and(warp::get())
        .map(|_id: String| warp::reply::json(&json!({ "messages": [] })));

    let resume_counter = resume_count.clone();
    let resume_store = sessions.clone();
    let resume_route = warp::path!("api" / "sessions" / String / "resume")
        .and(warp::post())
        .map(move |id: String| {
            resume_counter.fetch_add(1, Ordering::SeqCst);
            let items = resume_store.lock().unwrap();
            let found = items.iter().find(|s| s.id == id).cloned().unwrap();
            let body = serde_json::to_value(&found).unwrap();
            warp::reply::json(&body)
        });

    let routes = list_route.or(get_route).or(msgs_route).or(resume_route);
    let (addr, server_fut) = warp::serve(routes).bind_ephemeral(([127, 0, 0, 1], 0));
    let server = tokio::spawn(server_fut);

    let client = SharedBackendClient::new(addr.port()).unwrap();

    // Attach-mode guard rejects non-forwarded commands when a client is present.
    assert!(shared_proxy::reject_if_client(None, "agent_pause_workflow").is_ok());
    let guard_err =
        shared_proxy::reject_if_client(Some(&client), "agent_pause_workflow").unwrap_err();
    assert!(guard_err.contains("attached to the shared backend"));

    // Non-text content is rejected explicitly instead of dropped.
    let content_err = shared_proxy::extract_text_content(&[MCPContent::Image {
        data: Some("abc".to_string()),
        uri: None,
        mime_type: "image/png".to_string(),
    }])
    .unwrap_err();
    assert!(content_err.contains("Non-text message content"));

    // 404 -> Ok(None), 500 -> Err("boom"), 200 -> Ok(Some(SessionMetadata)).
    assert!(shared_proxy::get_session(&client, "missing")
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        shared_proxy::get_session(&client, "server-error")
            .await
            .unwrap_err(),
        "boom"
    );
    let found = shared_proxy::get_session(&client, "gui-session-1")
        .await
        .unwrap()
        .expect("session should exist");
    assert_eq!(found.id, "gui-session-1");

    // list_sessions + open_session map into AgentSessionListResponse / AgentOpenSessionResponse.
    let listed = shared_proxy::list_sessions(&client, None).await.unwrap();
    assert_eq!(listed.items.len(), 1);

    let opened = shared_proxy::open_session(&client, "gui-session-1", 40)
        .await
        .unwrap();
    assert_eq!(opened.session.id, "gui-session-1");
    assert!(opened.messages.items.is_empty());

    let resumed = client.resume_session("gui-session-1").await.unwrap();
    assert_eq!(resumed["id"], "gui-session-1");
    assert_eq!(resume_count.load(Ordering::SeqCst), 1);

    server.abort();
}
