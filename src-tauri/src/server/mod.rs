pub mod handlers;
pub mod mcp_handler;
pub mod routes;

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

/// Initialize and start the HTTP server.
/// If the requested port is in use, automatically fallback to subsequent available ports.
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
    let mut bound_port = requested_port;
    let mut bound_listener = None;

    for offset in 0..max_attempts {
        let attempt_port = requested_port.saturating_add(offset);
        match std::net::TcpListener::bind((bind_addr, attempt_port)) {
            Ok(listener) => {
                bound_port = attempt_port;
                bound_listener = Some(listener);
                if attempt_port != requested_port {
                    log::warn!(
                        "HTTP server port {} was in use; automatically fallback to port {}",
                        requested_port,
                        attempt_port
                    );
                }
                break;
            }
            Err(e) => {
                log::debug!("Port {} in use or unavailable: {}", attempt_port, e);
            }
        }
    }

    let listener = bound_listener.ok_or_else(|| {
        format!(
            "Failed to bind HTTP server on port range {}..{}",
            requested_port,
            requested_port.saturating_add(max_attempts)
        )
    })?;
    drop(listener);

    info!("Starting HTTP server on {}:{}", bind_addr, bound_port);

    set_active_http_port(bound_port);

    // Persist active HTTP port to ~/.libragent/http_port for external scripts/tools
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let dir = std::path::PathBuf::from(home).join(".libragent");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("http_port"), bound_port.to_string());
    }

    let routes = routes::get_routes(agent_manager, mcp_enabled, app_control_enabled);

    let server_future = warp::serve(routes).run((bind_addr.octets(), bound_port));

    // Spawn the server in a separate task so it doesn't block
    tokio::spawn(async move {
        server_future.await;
        info!("HTTP server stopped");
    });

    Ok(bound_port)
}
