pub mod builtin;
pub mod error_normalization;
pub mod integration_tests;
pub mod keychain;
pub mod oauth;
pub mod presets;
pub mod schema;
pub mod server;
pub mod server_utils;
pub mod service_proxy;
pub mod service_proxy_manager;
pub mod session_isolation;
pub mod session_isolation_config;
pub mod test_consistency;
pub mod types;
pub mod utils;

// Re-export common types to maintain backward compatibility and cleaner imports
pub use server::MCPServerManager;
pub use service_proxy_manager::MCPServiceProxyManager;
pub use session_isolation_config::SessionIsolationConfig;
pub use types::{
    MCPContent, MCPError, MCPPrompt, MCPResource, MCPResponse, MCPResult, MCPServerConfig, MCPTool,
    TransportConfig,
};
