pub mod handlers;
pub mod mcp_handler;
pub mod routes;

use log::info;
use std::sync::Arc;

use crate::agent::AgentSessionManager;

/// Initialize and start the HTTP server.
/// If the requested port is in use, automatically fallback to subsequent available ports.
pub async fn init(
    agent_manager: Arc<AgentSessionManager>,
    requested_port: u16,
    expose: bool,
    mcp_enabled: bool,
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

    // Persist active HTTP port to ~/.libragent/http_port for external scripts/tools
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let dir = std::path::PathBuf::from(home).join(".libragent");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("http_port"), bound_port.to_string());
    }

    let routes = routes::get_routes(agent_manager, mcp_enabled);

    let server_future = warp::serve(routes).run((bind_addr.octets(), bound_port));

    // Spawn the server in a separate task so it doesn't block
    tokio::spawn(async move {
        server_future.await;
        info!("HTTP server stopped");
    });

    Ok(bound_port)
}
