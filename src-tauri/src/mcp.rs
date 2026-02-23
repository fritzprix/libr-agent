// Re-export modular components for backward compatibility
pub use server::MCPServerManager;
pub use service_proxy_manager::MCPServiceProxyManager;
pub use types::{MCPError, MCPResponse, MCPTool, SamplingOptions, SamplingRequest};

// Session isolation types (used internally, not yet exported publicly)
#[allow(unused_imports)]
pub use session_isolation::SessionMCPManager;
#[allow(unused_imports)]
pub use session_isolation_config::SessionIsolationConfig;

pub mod builtin;
pub mod error_normalization;
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
pub mod types;
pub mod utils;

// This file now serves as a re-export hub for backward compatibility
// All implementation details have been moved to separate modules
