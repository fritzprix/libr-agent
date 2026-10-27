//! Wake-only listener for managed Docker sessions.
//!
//! The Session API listens on host loopback, which a container cannot reach. This second
//! listener serves nothing but `POST /api/sessions/:id/wake/:token`, so a container gets a
//! wake path without the rest of the Session API. The per-session token still authorizes
//! every call (see [`super::wake`]).

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use warp::Filter;

use crate::agent::AgentSessionManager;
use crate::server::handlers;

/// Host name a container uses to reach the host (see `--add-host` in the container runtime).
pub const CONTAINER_HOST_NAME: &str = "host.docker.internal";

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

/// Starts the wake-only listener on an ephemeral port and records the port.
pub fn start(manager: Arc<AgentSessionManager>) -> Result<u16, warp::Error> {
    let manager_filter = warp::any().map(move || manager.clone());
    let wake = warp::post()
        .and(warp::path("api"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("wake"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(manager_filter)
        .and(warp::body::content_length_limit(16 * 1024))
        .and(warp::body::bytes())
        .and_then(handlers::wake_session);

    let (addr, server) = warp::serve(wake).try_bind_ephemeral((bind_addr(), 0))?;
    WAKE_LISTENER_PORT.store(addr.port(), Ordering::Relaxed);
    tokio::spawn(server);
    Ok(addr.port())
}
