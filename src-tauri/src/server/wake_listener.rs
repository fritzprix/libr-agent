//! Wake-only HTTP listener for managed Docker sessions.
//!
//! Containers reach the host at `host.docker.internal:<port>`. They get this
//! listener and nothing else: only the wake route is registered, every other
//! path or method answers 404, and the main Session API server stays on
//! loopback. The port is assigned by the OS and is the one actually bound.
//! If the listener fails to start or stops, [`port`] is `None`, [`last_error`]
//! says why, and Docker sessions get no wake URL.

use crate::agent::AgentSessionManager;
use futures::Stream;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

/// Bind address. Open question (#2358): all interfaces vs. the Docker bridge
/// gateway; the wake token is the only credential either way.
const BIND_ADDR: Ipv4Addr = Ipv4Addr::UNSPECIFIED;

/// `0` = not running.
static PORT: AtomicU16 = AtomicU16::new(0);
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Port the listener is bound to, or `None` while it is not running.
pub fn port() -> Option<u16> {
    Some(PORT.load(Ordering::Relaxed)).filter(|port| *port != 0)
}

/// Why the listener is not running, if it failed to start or stopped.
pub fn last_error() -> Option<String> {
    LAST_ERROR.lock().ok().and_then(|error| error.clone())
}

fn record_failure(message: String) {
    PORT.store(0, Ordering::Relaxed);
    log::error!("{message}");
    if let Ok(mut error) = LAST_ERROR.lock() {
        *error = Some(message);
    }
}

fn incoming(listener: TcpListener) -> impl Stream<Item = std::io::Result<TcpStream>> {
    futures::stream::unfold(listener, |listener| async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => return Some((Ok(stream), listener)),
                Err(error) => {
                    log::warn!("wake listener accept failed: {error}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    })
}

/// Everything that is not a handled wake request answers 404; an oversized
/// body keeps its 413.
async fn only_wake_or_404(
    rejection: Rejection,
) -> Result<warp::reply::Response, std::convert::Infallible> {
    let status = if rejection.find::<warp::reject::PayloadTooLarge>().is_some() {
        StatusCode::PAYLOAD_TOO_LARGE
    } else {
        StatusCode::NOT_FOUND
    };
    let body = serde_json::json!({ "error": "not found" });
    Ok(warp::reply::with_status(warp::reply::json(&body), status).into_response())
}

fn routes(
    agent_manager: Arc<AgentSessionManager>,
) -> impl Filter<Extract = (impl Reply,), Error = std::convert::Infallible> + Clone {
    let manager = warp::any().map(move || agent_manager.clone());
    super::routes::wake_route(manager).recover(only_wake_or_404)
}

/// Binds an OS-assigned port and serves the wake route on it.
pub async fn start(agent_manager: Arc<AgentSessionManager>) -> Result<u16, String> {
    let result = bind_and_serve(agent_manager).await;
    if let Err(message) = &result {
        record_failure(message.clone());
    }
    result
}

async fn bind_and_serve(agent_manager: Arc<AgentSessionManager>) -> Result<u16, String> {
    let listener = TcpListener::bind((BIND_ADDR, 0))
        .await
        .map_err(|error| format!("wake listener could not bind: {error}"))?;
    let bound = listener
        .local_addr()
        .map_err(|error| format!("wake listener has no local address: {error}"))?
        .port();

    let server = warp::serve(routes(agent_manager)).serve_incoming(incoming(listener));
    PORT.store(bound, Ordering::Relaxed);
    if let Ok(mut error) = LAST_ERROR.lock() {
        *error = None;
    }
    log::info!("Wake listener serving on {BIND_ADDR}:{bound}");

    tokio::spawn(async move {
        server.await;
        record_failure("wake listener stopped".to_string());
    });
    Ok(bound)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same shape as the real wake route, without the session manager.
    fn stand_in() -> impl Filter<Extract = (impl Reply,), Error = std::convert::Infallible> + Clone
    {
        warp::post()
            .and(warp::path("api"))
            .and(warp::path("sessions"))
            .and(warp::path::param::<String>())
            .and(warp::path("wake"))
            .and(warp::path::param::<String>())
            .and(warp::path::end())
            .and(warp::body::content_length_limit(16))
            .map(|_id: String, _token: String| "woken")
            .recover(only_wake_or_404)
    }

    async fn status(method: &str, path: &str, body: &'static str) -> StatusCode {
        warp::test::request()
            .method(method)
            .path(path)
            .body(body)
            .reply(&stand_in())
            .await
            .status()
    }

    #[tokio::test]
    async fn only_the_wake_route_is_served() {
        assert_eq!(
            status("POST", "/api/sessions/s1/wake/tok", "").await,
            StatusCode::OK
        );
        for (method, path) in [
            ("GET", "/api/sessions"),
            ("POST", "/api/sessions"),
            ("GET", "/api/health"),
            ("POST", "/mcp"),
            ("POST", "/api/sessions/s1/messages"),
            ("GET", "/api/sessions/s1/wake/tok"),
            ("PUT", "/api/sessions/s1/wake/tok"),
            ("POST", "/api/sessions/s1/wake/tok/extra"),
        ] {
            assert_eq!(
                status(method, path, "").await,
                StatusCode::NOT_FOUND,
                "{method} {path}"
            );
        }
    }

    #[tokio::test]
    async fn oversized_wake_body_keeps_its_own_status() {
        assert_eq!(
            status(
                "POST",
                "/api/sessions/s1/wake/tok",
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
            )
            .await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[test]
    fn failure_clears_the_port_and_keeps_the_reason() {
        PORT.store(4242, Ordering::Relaxed);
        assert_eq!(port(), Some(4242));
        record_failure("boom".to_string());
        assert_eq!(port(), None);
        assert_eq!(last_error().as_deref(), Some("boom"));
    }
}
