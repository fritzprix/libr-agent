pub mod handlers;
pub mod routes;

use log::info;
use std::sync::Arc;

use crate::agent::AgentSessionManager;

/// Initialize and start the HTTP server
pub async fn init(
    agent_manager: Arc<AgentSessionManager>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting HTTP server on port {}", port);

    let routes = routes::get_routes(agent_manager);

    let server_future = warp::serve(routes).run(([127, 0, 0, 1], port));

    // Spawn the server in a separate task so it doesn't block
    tokio::spawn(async move {
        server_future.await;
        info!("HTTP server stopped");
    });

    Ok(())
}
