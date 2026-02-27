pub mod handlers;
pub mod mcp_handler;
pub mod routes;

use log::info;
use std::sync::Arc;

use crate::agent::AgentSessionManager;

/// Initialize and start the HTTP server
pub async fn init(
    agent_manager: Arc<AgentSessionManager>,
    port: u16,
    expose: bool,
    mcp_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let bind_addr = if expose {
        std::net::Ipv4Addr::UNSPECIFIED
    } else {
        std::net::Ipv4Addr::LOCALHOST
    };
    info!("Starting HTTP server on {}:{}", bind_addr, port);

    // Validate port binding up-front so startup failures can be propagated.
    let listener = std::net::TcpListener::bind((bind_addr, port))?;
    drop(listener);

    let routes = routes::get_routes(agent_manager, mcp_enabled);

    let server_future = warp::serve(routes).run((bind_addr.octets(), port));

    // Spawn the server in a separate task so it doesn't block
    tokio::spawn(async move {
        server_future.await;
        info!("HTTP server stopped");
    });

    Ok(())
}
