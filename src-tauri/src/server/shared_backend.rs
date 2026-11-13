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

/// Parsed payload from `GET /api/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub service: String,
    pub version: String,
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

impl SharedBackendClient {
    /// Build a client targeting `http://127.0.0.1:<port>`.
    ///
    /// Disables system proxy lookup so `HTTP_PROXY` / `ALL_PROXY` never
    /// hijack loopback frontend-to-backend traffic.
    pub fn new(port: u16) -> Self {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            port,
            base_url: format!("http://127.0.0.1:{port}"),
            client,
        }
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

    /// Query `GET /api/health` once.
    pub async fn health(&self) -> Result<HealthStatus, String> {
        let url = format!("{}/api/health", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to reach shared backend at {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "Shared backend health check returned HTTP {}",
                resp.status()
            ));
        }
        resp.json::<HealthStatus>()
            .await
            .map_err(|e| format!("Invalid /api/health response from shared backend: {e}"))
    }

    /// Wait until `GET /api/health` reports `ready: true`.
    ///
    /// A bare HTTP 200 (`status: "ok"`) is not enough: startup session recovery
    /// may still be running (`ready: false`, `recovery: "pending"`).
    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<HealthStatus, String> {
        let deadline = Instant::now() + timeout;

        loop {
            let last_error = match self.health().await {
                Ok(health) if health.ready => return Ok(health),
                Ok(health) => format!(
                    "backend not ready yet (status={}, recovery={})",
                    health.status, health.recovery
                ),
                Err(err) => err,
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

    async fn send_json(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
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
        if !status.is_success() {
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
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&text)
            .map_err(|e| format!("Invalid JSON from shared backend {url}: {e}"))
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
    let client = SharedBackendClient::new(descriptor.port);
    client.wait_until_ready(timeout).await?;
    Ok(Some((descriptor, client)))
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

        let client = SharedBackendClient::new(addr.port());

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
}
