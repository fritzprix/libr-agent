/// Session isolation module for MCP server management.
///
/// This module provides per-session isolation for external MCP servers,
/// ensuring each agent session has independent process instances to prevent
/// state conflicts during concurrent execution.
///
/// ## Architecture:
/// - **Stdio servers**: Independent processes per session (SessionMCPManager)
/// - **HTTP servers**: Shared connections with session ID injection (HttpSessionManager)
pub mod error;
pub mod http_manager;
pub mod process;
pub mod stdio_manager;

// Re-exports (not yet used externally, allowed for future integration)
#[allow(unused_imports)]
pub use error::SessionMCPError;
pub use http_manager::HttpSessionManager;
#[allow(unused_imports)]
pub use process::MCPProcess;
pub use stdio_manager::SessionMCPManager;
