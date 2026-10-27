//! Wake-only listener for managed Docker sessions.
//!
//! The Session API listens on host loopback, which a container cannot reach. This second
//! listener serves nothing but `POST /api/sessions/:id/wake/:token`, so a container gets a
//! wake path without the rest of the Session API. The per-session token still authorizes
//! every call (see [`super::wake`]), and only loopback and private-network sources are served.

use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

use crate::agent::AgentSessionManager;
use crate::server::handlers;

/// Host name a container uses to reach the host (see `--add-host` in the container runtime).
pub const CONTAINER_HOST_NAME: &str = "host.docker.internal";

/// Set once by [`start`]. Process-global: tests that rely on it being unset must not call `start`.
static WAKE_LISTENER_PORT: AtomicU16 = AtomicU16::new(0);

/// Port of the running wake-only listener, if it started.
pub fn wake_listener_port() -> Option<u16> {
    let port = WAKE_LISTENER_PORT.load(Ordering::Relaxed);
    (port != 0).then_some(port)
}

/// Linux containers reach the host through the bridge gateway, not host loopback, so the
/// listener must accept on all interfaces there. Docker Desktop (macOS, Windows) forwards
/// `host.docker.internal` to host loopback, so loopback is enough.
fn bind_addr() -> Ipv4Addr {
    if cfg!(target_os = "linux") {
        Ipv4Addr::UNSPECIFIED
    } else {
        Ipv4Addr::LOCALHOST
    }
}

/// Loopback and private-network sources only (RFC 1918, link-local, IPv6 loopback), so binding
/// all interfaces on Linux does not serve the public internet.
fn is_allowed_source(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// `POST /api/sessions/:id/wake/:token` from an allowed source, nothing else.
fn wake_routes<F>(manager: F) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    F: Filter<Extract = (Arc<AgentSessionManager>,), Error = Infallible>
        + Clone
        + Send
        + Sync
        + 'static,
{
    let allowed_source = warp::addr::remote()
        .and_then(|addr: Option<SocketAddr>| async move {
            match addr {
                Some(addr) if is_allowed_source(addr.ip()) => Ok(()),
                _ => Err(warp::reject::not_found()),
            }
        })
        .untuple_one();

    warp::post()
        .and(warp::path("api"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("wake"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(allowed_source)
        .and(manager)
        .and(warp::body::content_length_limit(16 * 1024))
        .and(warp::body::bytes())
        .and_then(handlers::wake_session)
}

/// Starts the wake-only listener on an ephemeral port and records the port.
pub fn start(manager: Arc<AgentSessionManager>) -> Result<u16, warp::Error> {
    let routes = wake_routes(warp::any().map(move || manager.clone()));
    let (addr, server) = warp::serve(routes).try_bind_ephemeral((bind_addr(), 0))?;
    WAKE_LISTENER_PORT.store(addr.port(), Ordering::Relaxed);
    tokio::spawn(server);
    Ok(addr.port())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// The manager filter is evaluated after the path, method and source filters, so rejected
    /// requests never build an `AgentSessionManager`.
    fn routes() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        wake_routes(warp::any().map(|| -> Arc<AgentSessionManager> {
            unreachable!("rejected requests must not reach the handler")
        }))
    }

    async fn status(method: &str, path: &str, remote: &str) -> u16 {
        warp::test::request()
            .method(method)
            .path(path)
            .remote_addr(remote.parse().unwrap())
            .reply(&routes())
            .await
            .status()
            .as_u16()
    }

    #[tokio::test]
    async fn only_the_wake_route_matches() {
        let wake = format!("/api/sessions/s1/wake/{TOKEN}");
        let src = "172.17.0.2:40000";
        for (method, path) in [
            ("POST", "/api/sessions/s1/messages".to_string()),
            ("GET", "/api/sessions/s1".to_string()),
            ("POST", "/api/sessions".to_string()),
            ("POST", format!("{wake}/extra")),
            ("POST", "/api/sessions/s1/wake".to_string()),
        ] {
            let code = status(method, &path, src).await;
            assert!(matches!(code, 404 | 405), "{method} {path}: {code}");
        }
        assert_ne!(
            status("GET", &wake, src).await,
            200,
            "GET wake must be rejected"
        );
    }

    #[tokio::test]
    async fn public_sources_are_not_served() {
        let wake = format!("/api/sessions/s1/wake/{TOKEN}");
        assert_eq!(status("POST", &wake, "8.8.8.8:40000").await, 404);
    }

    #[test]
    fn allowed_sources_are_loopback_and_private() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.17.0.2",
            "192.168.1.5",
            "169.254.1.1",
            "::1",
        ] {
            assert!(is_allowed_source(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "172.32.0.1", "2001:db8::1"] {
            assert!(!is_allowed_source(ip.parse().unwrap()), "{ip}");
        }
    }
}
