//! Shared-backend discovery, readiness wait, and Session API client (#2701).
//!
//! Both the GUI (`libragent`) and the TUI (`libragent --tui`) are equal frontends of one
//! local backend. Whichever starts first hosts the backend; a later frontend
//! discovers it through [`backend_descriptor::find_live_backend`], waits until
//! `/api/health` reports `ready: true`, and drives sessions exclusively through
//! the Session API so a live session never gets a second executor.

use crate::server::backend_descriptor::{self, BackendDescriptor};
use crate::server::frontend_attachments::{AttachResponse, HEARTBEAT_INTERVAL};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
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

    /// Register this frontend as attached to the backend (#2758).
    pub async fn attach_frontend(&self) -> Result<String, String> {
        let value = self
            .send_json(reqwest::Method::POST, "/api/frontends", None)
            .await?;
        serde_json::from_value::<AttachResponse>(value)
            .map(|resp| resp.id)
            .map_err(|e| format!("Invalid attach response from shared backend: {e}"))
    }

    /// Refresh this frontend's attachment. `Ok(false)` means the backend no
    /// longer knows the id (it dropped it as stale), so attach again.
    pub async fn heartbeat_frontend(&self, attachment_id: &str) -> Result<bool, String> {
        let (status, _) = self
            .send_json_status(
                reqwest::Method::POST,
                &format!("/api/frontends/{attachment_id}/heartbeat"),
                None,
            )
            .await?;
        Ok(status != reqwest::StatusCode::NOT_FOUND)
    }

    pub async fn detach_frontend(&self, attachment_id: &str) -> Result<(), String> {
        self.send_json_status(
            reqwest::Method::DELETE,
            &format!("/api/frontends/{attachment_id}"),
            None,
        )
        .await
        .map(|_| ())
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

/// This process's attachment id on the shared backend, while it holds one.
static ATTACHMENT_ID: Mutex<Option<String>> = Mutex::new(None);

fn attachment_id() -> std::sync::MutexGuard<'static, Option<String>> {
    ATTACHMENT_ID
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Keep this frontend counted by the backend (#2758): attach, then send a
/// heartbeat every [`HEARTBEAT_INTERVAL`]. When the backend no longer knows
/// the id (for example after this process was suspended past the timeout),
/// attach again. Runs until the process exits.
pub async fn hold_frontend_attachment(client: SharedBackendClient) {
    loop {
        let current = attachment_id().clone();
        let next = attachment_step(&client, current).await;
        *attachment_id() = next;
        tokio::time::sleep(HEARTBEAT_INTERVAL).await;
    }
}

/// One heartbeat-loop step: the attachment id to hold after it. No id or an
/// id the backend no longer knows means attach (again); a failed request
/// keeps the current id so the next step retries it.
async fn attachment_step(client: &SharedBackendClient, current: Option<String>) -> Option<String> {
    let known = match &current {
        Some(id) => client.heartbeat_frontend(id).await,
        None => Ok(false),
    };
    match known {
        Ok(true) => current,
        Ok(false) => match client.attach_frontend().await {
            Ok(id) => {
                log::info!("Attached to shared backend as frontend {id}");
                Some(id)
            }
            Err(err) => {
                log::warn!("Failed to attach to shared backend: {err}");
                None
            }
        },
        Err(err) => {
            log::warn!("Shared backend heartbeat failed: {err}");
            current
        }
    }
}

/// Release this frontend's attachment on clean exit. A crash skips this; the
/// backend then drops the attachment when heartbeats stop.
pub async fn detach_frontend(client: &SharedBackendClient) {
    let Some(id) = attachment_id().take() else {
        return;
    };
    if let Err(err) = client.detach_frontend(&id).await {
        log::warn!("Failed to detach frontend {id} from shared backend: {err}");
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

/// How long the winner of the startup race waits for its own HTTP server to
/// publish this process's backend record before releasing the startup lock
/// (#2757). Binding is fast; the wait only matters when binding fails, and
/// it keeps the loser from starting a second backend while a winner that is
/// still publishing holds the lock.
pub const OWN_PUBLISH_TIMEOUT: Duration = Duration::from_secs(10);

/// The single backend startup path every frontend shares (#2756), guarded
/// against two frontends starting at the same moment (#2757).
///
/// Fast path: a live published backend exists, so attach to it and wait for
/// real readiness (`ready: true`). Otherwise the check-then-start sequence is
/// serialized on `lock_path`: the winner re-checks for a backend that
/// appeared while it waited, then runs `start_backend` to host one here
/// (HTTP server, scheduler, session recovery) and waits for this process's
/// own record to go live before releasing the lock. The loser waits on the
/// lock and then finds the winner's record, so it attaches instead of
/// starting a second HTTP server, scheduler, or session executor.
///
/// `lock_path` is `None` before the descriptor namespace is initialized: a
/// single-process fallback with the pre-#2757 behavior.
///
/// A published backend that fails readiness is a hard error: its process
/// already owns this profile, so starting a second backend here would
/// double-execute live sessions.
async fn attach_or_start_backend_with_race<S>(
    timeout: Duration,
    lock_path: Option<PathBuf>,
    find_live: impl Fn() -> Option<BackendDescriptor>,
    start_backend: S,
    wait_own_publish: impl Fn(Duration),
) -> Result<BackendStartup, String>
where
    S: FnOnce() -> Result<(), String>,
{
    if let Some(descriptor) = find_live() {
        return attach_to_descriptor(&descriptor, timeout).await;
    }

    let Some(path) = lock_path else {
        start_backend()?;
        return Ok(BackendStartup::Started);
    };

    let guard = crate::server::startup_lock::acquire(&path, Instant::now() + timeout).await?;

    // Under the lock: the winner of the race may have published (and
    // released) while this process waited. Attach instead of starting.
    if let Some(descriptor) = find_live() {
        return attach_to_descriptor(&descriptor, timeout).await;
    }

    start_backend()?;
    // Release the lock only once this process's record is live (or the wait
    // gave up), so the loser's re-check under the lock sees this backend.
    wait_own_publish(OWN_PUBLISH_TIMEOUT);
    drop(guard);
    Ok(BackendStartup::Started)
}

/// Attach to a discovered backend and wait until it reports readiness.
async fn attach_to_descriptor(
    descriptor: &BackendDescriptor,
    timeout: Duration,
) -> Result<BackendStartup, String> {
    let client = SharedBackendClient::new(descriptor.port)?;
    client.wait_until_ready(timeout).await.map_err(|err| {
        format!(
            "Shared backend pid {} at port {} did not become ready: {err}",
            descriptor.pid, descriptor.port
        )
    })?;
    Ok(BackendStartup::Attached {
        descriptor: descriptor.clone(),
        client,
    })
}

/// The production entry: race on the profile's startup lock, prove holder
/// death with the real process probes, and wait for this process's own
/// descriptor record before releasing the lock.
pub async fn attach_or_start_backend<S>(
    timeout: Duration,
    start_backend: S,
) -> Result<BackendStartup, String>
where
    S: FnOnce() -> Result<(), String>,
{
    attach_or_start_backend_with_race(
        timeout,
        backend_descriptor::startup_lock_path(),
        backend_descriptor::find_live_backend,
        start_backend,
        backend_descriptor::wait_until_own_record_live,
    )
    .await
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
    async fn client_attaches_heartbeats_and_detaches() {
        let (addr, server_fut) = warp::serve(crate::server::frontend_attachments::routes())
            .bind_ephemeral(([127, 0, 0, 1], 0));
        let server = tokio::spawn(server_fut);
        let client = SharedBackendClient::new(addr.port()).unwrap();

        let id = client.attach_frontend().await.expect("attach succeeds");
        assert!(client.heartbeat_frontend(&id).await.unwrap());

        client.detach_frontend(&id).await.expect("detach succeeds");
        assert!(
            !client.heartbeat_frontend(&id).await.unwrap(),
            "a detached id must be unknown, so the frontend attaches again"
        );
        server.abort();
    }

    fn attachment_server() -> (SharedBackendClient, tokio::task::JoinHandle<()>) {
        let (addr, server_fut) = warp::serve(crate::server::frontend_attachments::routes())
            .bind_ephemeral(([127, 0, 0, 1], 0));
        let server = tokio::spawn(server_fut);
        (SharedBackendClient::new(addr.port()).unwrap(), server)
    }

    #[tokio::test]
    async fn attachment_step_attaches_when_no_id_is_held() {
        let (client, server) = attachment_server();
        let id = attachment_step(&client, None).await.expect("attached");
        assert!(client.heartbeat_frontend(&id).await.unwrap());
        server.abort();
    }

    #[tokio::test]
    async fn attachment_step_keeps_a_known_id() {
        let (client, server) = attachment_server();
        let id = client.attach_frontend().await.unwrap();
        assert_eq!(attachment_step(&client, Some(id.clone())).await, Some(id));
        server.abort();
    }

    #[tokio::test]
    async fn attachment_step_reattaches_when_the_backend_forgot_the_id() {
        let (client, server) = attachment_server();
        let next = attachment_step(&client, Some("dropped-as-stale".to_string()))
            .await
            .expect("re-attached");
        assert_ne!(next, "dropped-as-stale");
        assert!(client.heartbeat_frontend(&next).await.unwrap());
        server.abort();
    }

    #[tokio::test]
    async fn attachment_step_keeps_the_id_when_the_request_fails() {
        let (client, server) = attachment_server();
        server.abort();
        let _ = server.await;
        let kept = attachment_step(&client, Some("held".to_string())).await;
        assert_eq!(kept.as_deref(), Some("held"));
    }

    #[tokio::test]
    async fn startup_starts_locally_when_no_live_backend_exists() {
        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();

        let outcome = attach_or_start_backend_with_race(
            Duration::from_secs(1),
            None,
            || None,
            move || {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            },
            |_| {},
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

        let outcome = attach_or_start_backend_with_race(
            Duration::from_secs(2),
            None,
            move || Some(live_descriptor(port)),
            move || {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            },
            |_| {},
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

        let err = attach_or_start_backend_with_race(
            Duration::from_millis(250),
            None,
            move || Some(live_descriptor(port)),
            move || {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            },
            |_| {},
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

    fn temp_lock_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("profile.db.backend.lock")
    }

    /// A real live process the OS probe reports as alive for the whole test.
    fn live_holder_child() -> std::process::Child {
        if cfg!(windows) {
            std::process::Command::new("ping")
                .args(["-n", "30", "127.0.0.1"])
                .stdout(std::process::Stdio::null())
                .spawn()
                .expect("spawn a live holder stand-in")
        } else {
            std::process::Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("spawn a live holder stand-in")
        }
    }

    /// #2757: while the startup lock is held by a live process, the loser
    /// waits; once the holder releases it and its record is live, the loser
    /// attaches and never starts its own backend.
    #[tokio::test]
    async fn startup_race_attaches_to_the_backend_published_by_the_lock_holder() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_lock_path(&dir);
        let (port, server) = health_server(true);
        let published = Arc::new(AtomicBool::new(false));
        let started = Arc::new(AtomicBool::new(false));

        let mut holder = live_holder_child();
        std::fs::write(
            &path,
            serde_json::json!({ "pid": holder.id(), "acquired_at_ms": 1 }).to_string(),
        )
        .unwrap();

        let published_for_finder = published.clone();
        let started_for_start = started.clone();
        let path_for_race = path.clone();
        let race = tokio::spawn(async move {
            attach_or_start_backend_with_race(
                Duration::from_secs(5),
                Some(path_for_race),
                move || {
                    published_for_finder
                        .load(Ordering::Relaxed)
                        .then(|| live_descriptor(port))
                },
                move || {
                    started_for_start.store(true, Ordering::Relaxed);
                    Ok(())
                },
                |_| {},
            )
            .await
        });

        // The loser must wait on the lock even though no backend exists yet.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(
            !started.load(Ordering::Relaxed),
            "the loser must not start a backend while the lock is held"
        );

        // The holder publishes its backend, then releases the lock.
        published.store(true, Ordering::Relaxed);
        let _ = holder.kill();
        std::fs::remove_file(&path).unwrap();

        let outcome = race
            .await
            .unwrap()
            .expect("the loser attaches after the holder");
        assert!(
            matches!(outcome, BackendStartup::Attached { descriptor, .. } if descriptor.port == port),
            "the loser must attach to the published backend"
        );
        assert!(
            !started.load(Ordering::Relaxed),
            "start_backend must not run for the loser"
        );
        server.abort();
    }

    /// #2757: a lock left by a definitively-dead process is swept, and the
    /// next frontend starts the one backend.
    #[tokio::test]
    async fn startup_race_takes_over_after_sweeping_a_dead_holder() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_lock_path(&dir);
        std::fs::write(
            &path,
            serde_json::json!({ "pid": u32::MAX, "acquired_at_ms": 1 }).to_string(),
        )
        .unwrap();

        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();
        let outcome = attach_or_start_backend_with_race(
            Duration::from_secs(2),
            Some(path.clone()),
            || None,
            move || {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            },
            |_| {},
        )
        .await
        .expect("a dead holder must not block startup");

        assert!(matches!(outcome, BackendStartup::Started));
        assert!(started.load(Ordering::Relaxed));
        assert!(!path.exists(), "the guard must release the lock");
    }

    /// #2757: a live holder that never publishes blocks the second frontend
    /// until the timeout; a blocked startup never starts a second backend.
    #[tokio::test]
    async fn startup_race_gives_up_while_a_live_process_holds_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_lock_path(&dir);

        let mut child = live_holder_child();
        std::fs::write(
            &path,
            serde_json::json!({ "pid": child.id(), "acquired_at_ms": 1 }).to_string(),
        )
        .unwrap();

        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();
        let err = attach_or_start_backend_with_race(
            Duration::from_millis(300),
            Some(path),
            || None,
            move || {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            },
            |_| {},
        )
        .await
        .expect_err("a live holder must block a second backend");

        let _ = child.kill();
        assert!(err.contains("startup lock"), "unexpected error: {err}");
        assert!(
            !started.load(Ordering::Relaxed),
            "a blocked startup must never start a second backend"
        );
    }
}
