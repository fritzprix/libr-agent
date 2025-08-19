pub mod interactive_browser_server;

pub use interactive_browser_server::{BrowserSession, InteractiveBrowserServer};

use std::sync::OnceLock;

// Global browser server instance
static BROWSER_SERVER: OnceLock<InteractiveBrowserServer> = OnceLock::new();

pub fn get_browser_server() -> &'static InteractiveBrowserServer {
    BROWSER_SERVER
        .get()
        .expect("Browser server not initialized. Call this only after app setup.")
}

pub fn initialize_browser_server(server: InteractiveBrowserServer) {
    BROWSER_SERVER
        .set(server)
        .expect("Browser server already initialized");
}
