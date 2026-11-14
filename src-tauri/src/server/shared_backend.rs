//! Shared-backend discovery, readiness wait, and Session API client (#2701).
//!
//! Both the GUI (`libragent`) and the TUI (`liblet`) are equal frontends of one
//! local backend. Whichever starts first hosts the backend; a later frontend
//! discovers it through [`backend_descriptor::find_live_backend`], waits until
//! `/api/health` reports `ready: true`, and drives sessions exclusively through
//! the Session API so a live session never gets a second executor.

use crate::server::backend_descriptor::{self, BackendDescriptor};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a frontend waits for a discovered backend to report `ready: true`
/// on the shared startup path (#2756).
pub const ATTACH_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Parsed payload from `GET /api/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub service: String,
    pub version: String,
    #[serde(default)]
    pub build_sha: Option<String>,
    #[serde(default)]
    pub build_dirty: Option<bool>,
    pub ready: bool,
    pub recovery: String,
}

/// HTTP client for a running local Session API backend.
#[derive(Debug, Clone)]
pub struct SharedBackendClient {
    port: u16,
    base_url: String,
    client: reqwest::Client,
}

/// Outcome of a single health probe. A 401 is permanent for a keyless client
/// (the backend is key-guarded), so it must not be retried like a transient
/// state.
enum HealthProbeError {
    Unauthorized(String),
    Other(String),
}

/// Shared 401 wording so the health and JSON paths report the same failure.
fn unauthorized_message(url: &str) -> String {
    format!(
        "Shared backend {url} rejected the request (401 Unauthorized): Session API key required or invalid"
    )
}

impl SharedBackendClient {
    /// Build a client targeting `http://127.0.0.1:<port>`.
    ///
    /// Disables system proxy lookup so `HTTP_PROXY` / `ALL_PROXY` never
    /// hijack loopback frontend-to-backend traffic.
    pub fn new(port: u16) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| format!("Failed to build shared backend HTTP client: {e}"))?;
        Ok(Self {
            port,
            base_url: format!("http://127.0.0.1:{port}"),
            client,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn events_url(&self, session_id: &str) -> String {
        format!("{}/api/sessions/{session_id}/events", self.base_url)
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.client
    }

    async fn health_probe(&self) -> Result<HealthStatus, HealthProbeError> {
        let url = format!("{}/api/health", self.base_url);
        let resp = self.client.get(&url).send().await.map_err(|e| {
            HealthProbeError::Other(format!("Failed to reach shared backend at {url}: {e}"))
        })?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(HealthProbeError::Unauthorized(unauthorized_message(&url)));
        }
        if !resp.status().is_success() {
            return Err(HealthProbeError::Other(format!(
                "Shared backend health check returned HTTP {}",
                resp.status()
            )));
        }
        resp.json::<HealthStatus>().await.map_err(|e| {
            HealthProbeError::Other(format!(
                "Invalid /api/health response from shared backend: {e}"
            ))
        })
    }

    /// Wait until `GET /api/health` reports `ready: true`.
    ///
    /// A bare HTTP 200 (`status: "ok"`) is not enough: startup session recovery
    /// may still be running (`ready: false`, `recovery: "pending"`).
    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<HealthStatus, String> {
        let deadline = Instant::now() + timeout;

        loop {
            let last_error = match self.health_probe().await {
                Ok(health) if health.ready => return Ok(health),
                Ok(health) => format!(
                    "backend not ready yet (status={}, recovery={})",
                    health.status, health.recovery
                ),
                // A 401 never resolves by retrying: fail fast with the
                // explicit message instead of burning the whole timeout.
                Err(HealthProbeError::Unauthorized(msg)) => return Err(msg),
                Err(HealthProbeError::Other(err)) => err,
            };

            if Instant::now() >= deadline {
                return Err(format!(
                    "Timed out after {:?} waiting for shared backend readiness at {}: {}",
                    timeout, self.base_url, last_error
                ));
            }
            tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
        }
    }

    async fn send_json_status(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(reqwest::StatusCode, serde_json::Value), String> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);
        if let Some(payload) = body {
            req = req.json(payload);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| format!("Shared backend request {url} failed: {e}"))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed reading shared backend response from {url}: {e}"))?;
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(unauthorized_message(&url));
        }
        if !status.is_success() {
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok((status, serde_json::Value::Null));
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(msg) = json.get("error").and_then(|v| v.as_str()) {
                    return Err(msg.to_string());
                }
            }
            return Err(format!(
                "Shared backend {url} returned HTTP {status}: {text}"
            ));
        }
        if text.trim().is_empty() {
            return Ok((status, serde_json::Value::Null));
        }
        let parsed = serde_json::from_str(&text)
            .map_err(|e| format!("Invalid JSON from shared backend {url}: {e}"))?;
        Ok((status, parsed))
    }

    async fn send_json(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let (status, val) = self.send_json_status(method, path, body).await?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(format!(
                "Shared backend {}{} returned HTTP 404 Not Found",
                self.base_url, path
            ));
        }
        Ok(val)
    }

    pub async fn list_sessions(&self, limit: Option<u64>) -> Result<serde_json::Value, String> {
        let path = match limit {
            Some(l) => format!("/api/sessions?limit={l}"),
            None => "/api/sessions".to_string(),
        };
        self.send_json(reqwest::Method::GET, &path, None).await
    }

    pub async fn create_session(
        &self,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.send_json(reqwest::Method::POST, "/api/sessions", Some(body))
            .await
    }

    pub async fn get_session(&self, session_id: &str) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/sessions/{session_id}"),
            None,
        )
        .await
    }

    pub async fn get_session_opt(
        &self,
        session_id: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        let (status, val) = self
            .send_json_status(
                reqwest::Method::GET,
                &format!("/api/sessions/{session_id}"),
                None,
            )
            .await?;
        if status == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            Ok(Some(val))
        }
    }

    pub async fn patch_session(
        &self,
        session_id: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/sessions/{session_id}"),
            Some(body),
        )
        .await
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/sessions/{session_id}"),
            None,
        )
        .await
    }

    pub async fn resume_session(&self, session_id: &str) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/sessions/{session_id}/resume"),
            Some(&serde_json::json!({})),
        )
        .await
    }

    pub async fn terminate_session(&self, session_id: &str) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/sessions/{session_id}/terminate"),
            Some(&serde_json::json!({})),
        )
        .await
    }

    pub async fn get_messages(
        &self,
        session_id: &str,
        limit: Option<u64>,
    ) -> Result<serde_json::Value, String> {
        let path = match limit {
            Some(l) => format!("/api/sessions/{session_id}/messages?limit={l}"),
            None => format!("/api/sessions/{session_id}/messages"),
        };
        self.send_json(reqwest::Method::GET, &path, None).await
    }

    pub async fn send_message(
        &self,
        session_id: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/sessions/{session_id}/messages"),
            Some(body),
        )
        .await
    }

    pub async fn inject_channel_message(
        &self,
        session_id: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/sessions/{session_id}/channel/messages"),
            Some(body),
        )
        .await
    }

    pub async fn respond_permission(
        &self,
        session_id: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/sessions/{session_id}/channel/permission"),
            Some(body),
        )
        .await
    }

    pub async fn respond_prompt(
        &self,
        session_id: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/sessions/{session_id}/channel/prompt"),
            Some(body),
        )
        .await
    }
}

/// Discover a live backend for the active profile and wait until its
/// `/api/health` endpoint reports `ready: true`.
///
/// Returns `Ok(None)` when no live backend is running.
pub async fn attach_to_live_backend(
    timeout: Duration,
) -> Result<Option<(BackendDescriptor, SharedBackendClient)>, String> {
    let Some(descriptor) = backend_descriptor::find_live_backend() else {
        return Ok(None);
    };
    let client = SharedBackendClient::new(descriptor.port)?;
    client.wait_until_ready(timeout).await?;
    Ok(Some((descriptor, client)))
}

/// How this frontend reached a backend through the shared startup path
/// (#2756). Every frontend (the GUI now, `libragent --tui` later) enters the
/// backend the same way.
#[derive(Debug)]
pub enum BackendStartup {
    /// Another live process hosts the backend. Drive it through the returned
    /// client; this process must not start a second HTTP server, scheduler,
    /// or session executor.
    Attached {
        descriptor: BackendDescriptor,
        client: SharedBackendClient,
    },
    /// No live backend was published, so `start_backend` ran and this process
    /// now hosts the backend.
    Started,
}

/// The single backend startup path every frontend shares (#2756): attach to
/// a live published backend and wait for real readiness (`ready: true`), or
/// run `start_backend` to host one here (HTTP server, scheduler, session
/// recovery) and then present the UI.
///
/// A published backend that fails readiness is a hard error: its process
/// already owns this profile, so starting a second backend here would
/// double-execute live sessions.
pub async fn attach_or_start_backend<S>(
    timeout: Duration,
    start_backend: S,
) -> Result<BackendStartup, String>
where
    S: FnOnce() -> Result<(), String>,
{
    attach_or_start_backend_with_finder(
        timeout,
        backend_descriptor::find_live_backend,
        start_backend,
    )
    .await
}

async fn attach_or_start_backend_with_finder<S, F>(
    timeout: Duration,
    find_live: F,
    start_backend: S,
) -> Result<BackendStartup, String>
where
    F: FnOnce() -> Option<BackendDescriptor>,
    S: FnOnce() -> Result<(), String>,
{
    let Some(descriptor) = find_live() else {
        start_backend()?;
        return Ok(BackendStartup::Started);
    };
    let client = SharedBackendClient::new(descriptor.port)?;
    client.wait_until_ready(timeout).await.map_err(|err| {
        format!(
            "Shared backend pid {} at port {} did not become ready: {err}",
            descriptor.pid, descriptor.port
        )
    })?;
    Ok(BackendStartup::Attached { descriptor, client })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use warp::Filter;

    #[tokio::test]
    async fn wait_until_ready_ignores_ok_until_ready_is_true() {
        let ready = Arc::new(AtomicBool::new(false));
        let ready_route = ready.clone();

        let route = warp::path!("api" / "health").and(warp::get()).map(move || {
            let is_ready = ready_route.load(Ordering::Relaxed);
            warp::reply::json(&serde_json::json!({
                "status": "ok",
                "service": "libr-agent-session-api",
                "version": "0.9.0",
                "ready": is_ready,
                "recovery": if is_ready { "ok" } else { "pending" }
            }))
        });

        let (addr, server_fut) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
        let server = tokio::spawn(server_fut);

        let client = SharedBackendClient::new(addr.port()).unwrap();

        // Must time out while ready == false even though status == "ok".
        let err = client
            .wait_until_ready(Duration::from_millis(250))
            .await
            .unwrap_err();
        assert!(err.contains("not ready yet"), "unexpected error: {err}");

        // Flip ready -> true and verify wait_until_ready succeeds.
        let flip = ready.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(120)).await;
            flip.store(true, Ordering::Relaxed);
        });

        let status = client
            .wait_until_ready(Duration::from_secs(2))
            .await
            .expect("should become ready");
        assert!(status.ready);
        assert_eq!(status.recovery, "ok");

        server.abort();
    }

    fn live_descriptor(port: u16) -> BackendDescriptor {
        BackendDescriptor {
            pid: 999_999,
            port,
            started_at_ms: 1,
        }
    }

    /// A stand-in backend whose `/api/health` always answers with `ready`.
    fn health_server(ready: bool) -> (u16, tokio::task::JoinHandle<()>) {
        let route = warp::path!("api" / "health").and(warp::get()).map(move || {
            warp::reply::json(&serde_json::json!({
                "status": "ok",
                "service": "libr-agent-session-api",
                "version": "0.9.0",
                "ready": ready,
                "recovery": if ready { "ok" } else { "pending" }
            }))
        });
        let (addr, server_fut) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
        let server = tokio::spawn(server_fut);
        (addr.port(), server)
    }

    #[tokio::test]
    async fn startup_starts_locally_when_no_live_backend_exists() {
        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();

        let outcome = attach_or_start_backend_with_finder(
            Duration::from_secs(1),
            || None,
            move || {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            },
        )
        .await
        .expect("startup succeeds");

        assert!(matches!(outcome, BackendStartup::Started));
        assert!(
            started.load(Ordering::Relaxed),
            "start_backend must run when no backend is live"
        );
    }

    #[tokio::test]
    async fn startup_attaches_to_a_ready_backend_without_starting_one() {
        let (port, server) = health_server(true);
        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();

        let outcome = attach_or_start_backend_with_finder(
            Duration::from_secs(2),
            move || Some(live_descriptor(port)),
            move || {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            },
        )
        .await
        .expect("attach succeeds");

        match outcome {
            BackendStartup::Attached { descriptor, client } => {
                assert_eq!(descriptor.port, port);
                assert_eq!(client.port(), port);
            }
            BackendStartup::Started => panic!("must attach to the live backend, not start one"),
        }
        assert!(
            !started.load(Ordering::Relaxed),
            "start_backend must not run when attaching"
        );
        server.abort();
    }

    #[tokio::test]
    async fn startup_fails_hard_when_a_live_backend_never_becomes_ready() {
        let (port, server) = health_server(false);
        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();

        let err = attach_or_start_backend_with_finder(
            Duration::from_millis(250),
            move || Some(live_descriptor(port)),
            move || {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            },
        )
        .await
        .unwrap_err();

        assert!(
            err.contains("did not become ready"),
            "unexpected error: {err}"
        );
        assert!(
            !started.load(Ordering::Relaxed),
            "a failed attach must never start a second backend"
        );
        server.abort();
    }
}
