pub mod access_guard;
pub mod api_key;
#[cfg(debug_assertions)]
pub mod e2e_control;
pub mod handlers;
pub mod mcp_handler;
pub mod pairing;
pub mod readiness;
pub mod routes;
pub mod wake;
pub mod wake_listener;

use log::info;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use crate::agent::AgentSessionManager;

/// Process-wide bound HTTP port (`0` = not set yet).
static ACTIVE_HTTP_PORT: AtomicU16 = AtomicU16::new(0);

/// Record the port the HTTP server actually bound (may differ from Settings request).
pub fn set_active_http_port(port: u16) {
    ACTIVE_HTTP_PORT.store(port, Ordering::Relaxed);
}

/// Active LibrAgent HTTP port for external inject / wake URLs.
///
/// Prefers the in-process bound port, then `~/.libragent/http_port`, else **3030**.
pub fn active_http_port() -> u16 {
    let bound = ACTIVE_HTTP_PORT.load(Ordering::Relaxed);
    if bound != 0 {
        return bound;
    }
    read_persisted_http_port().unwrap_or(3030)
}

/// `~/.libragent/http_port` is shared by every instance under one HOME. An instance
/// with an overridden data dir (e.g. an isolated bench instance) must not clobber the
/// primary instance's port.
fn should_persist_global_http_port(data_dir_override: Option<&std::ffi::OsStr>) -> bool {
    data_dir_override.is_none_or(|value| value.is_empty())
}

fn read_persisted_http_port() -> Option<u16> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    let raw = std::fs::read_to_string(
        std::path::PathBuf::from(home)
            .join(".libragent")
            .join("http_port"),
    )
    .ok()?;
    let port: u16 = raw.trim().parse().ok()?;
    (port > 0).then_some(port)
}

/// Bind the first free listener in `requested_port..requested_port + max_attempts`.
///
/// `requested_port == 0` asks the OS for any free port; the caller must read
/// the real port back from the returned listener (`local_addr()`).
fn bind_first_available_listener(
    bind_addr: std::net::Ipv4Addr,
    requested_port: u16,
    max_attempts: u16,
) -> Result<std::net::TcpListener, String> {
    for offset in 0..max_attempts {
        let attempt_port = requested_port.saturating_add(offset);
        match std::net::TcpListener::bind((bind_addr, attempt_port)) {
            Ok(listener) => {
                if attempt_port != requested_port {
                    log::warn!(
                        "HTTP server port {} was in use; automatically fallback to port {}",
                        requested_port,
                        attempt_port
                    );
                }
                return Ok(listener);
            }
            Err(e) => {
                log::debug!("Port {} in use or unavailable: {}", attempt_port, e);
            }
        }
    }
    Err(format!(
        "Failed to bind HTTP server on port range {}..{}",
        requested_port,
        requested_port.saturating_add(max_attempts)
    ))
}

/// Initialize and start the HTTP server.
/// If the requested port is in use, automatically fallback to subsequent available ports.
/// Port `0` binds an OS-assigned port; the returned value is the real port.
pub async fn init(
    agent_manager: Arc<AgentSessionManager>,
    requested_port: u16,
    expose: bool,
    mcp_enabled: bool,
    app_control_enabled: bool,
) -> Result<u16, Box<dyn std::error::Error>> {
    let bind_addr = if expose {
        std::net::Ipv4Addr::UNSPECIFIED
    } else {
        std::net::Ipv4Addr::LOCALHOST
    };

    let max_attempts = 10u16;
    // Keep the bound listener and serve through it: dropping it and asking
    // warp to re-bind the same port left a TOCTOU window where another
    // process could steal the port between the probe and the real bind.
    let listener = bind_first_available_listener(bind_addr, requested_port, max_attempts)?;
    let bound_port = listener
        .local_addr()
        .map_err(|e| format!("Failed to read bound HTTP port: {e}"))?
        .port();

    info!("Starting HTTP server on {}:{}", bind_addr, bound_port);

    set_active_http_port(bound_port);

    // Persist active HTTP port to ~/.libragent/http_port for external scripts/tools
    if should_persist_global_http_port(std::env::var_os("LIBRAGENT_DATA_DIR").as_deref()) {
        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            let dir = std::path::PathBuf::from(home).join(".libragent");
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("http_port"), bound_port.to_string());
        }
    }

    #[cfg(debug_assertions)]
    crate::server::e2e_control::init_capability_token();

    let routes = routes::get_routes(agent_manager, mcp_enabled, app_control_enabled, expose);

    // Serve through the already-bound listener. Letting warp bind the port
    // itself would reopen the probe-then-rebind race the helper above closes.
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set HTTP listener non-blocking: {e}"))?;
    let listener = tokio::net::TcpListener::from_std(listener)
        .map_err(|e| format!("Failed to adopt HTTP listener: {e}"))?;

    // Spawn the server in a separate task so it doesn't block
    tokio::spawn(async move {
        let incoming = futures::stream::unfold(listener, |listener| async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        // Match warp's own bind path, which sets nodelay on accepted sockets.
                        let _ = stream.set_nodelay(true);
                        break Some((Ok::<_, std::io::Error>(stream), listener));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        log::warn!("HTTP accept failed, stopping server: {e}");
                        break None;
                    }
                }
            }
        });
        warp::serve(routes).serve_incoming(incoming).await;
        info!("HTTP server stopped");
    });

    Ok(bound_port)
}

#[cfg(test)]
mod tests {
    use super::{bind_first_available_listener, should_persist_global_http_port};
    use std::ffi::OsStr;
    use std::net::Ipv4Addr;

    #[test]
    fn port_zero_advertises_os_assigned_port() {
        let listener =
            bind_first_available_listener(Ipv4Addr::LOCALHOST, 0, 1).expect("port 0 should bind");
        let port = listener.local_addr().expect("local_addr").port();
        assert_ne!(port, 0, "port 0 must advertise the real OS-assigned port");
    }

    #[test]
    fn occupied_port_falls_back_to_next_free() {
        let first = bind_first_available_listener(Ipv4Addr::LOCALHOST, 0, 1).expect("seed bind");
        let first_port = first.local_addr().expect("seed local_addr").port();

        let second = bind_first_available_listener(Ipv4Addr::LOCALHOST, first_port, 10)
            .expect("fallback bind");
        let second_port = second.local_addr().expect("fallback local_addr").port();
        assert_ne!(second_port, first_port, "occupied port must fall back");
    }

    #[test]
    fn default_data_dir_persists_global_port() {
        assert!(should_persist_global_http_port(None));
        assert!(should_persist_global_http_port(Some(OsStr::new(""))));
    }

    #[test]
    fn overridden_data_dir_does_not_clobber_global_port() {
        assert!(!should_persist_global_http_port(Some(OsStr::new(
            "/tmp/bench/com.fritzprix.libragent"
        ))));
    }
}
