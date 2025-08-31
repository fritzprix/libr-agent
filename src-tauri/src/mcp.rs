// Re-export modular components for backward compatibility
pub use server::MCPServerManager;
pub use types::{
    MCPError, MCPResponse, MCPServerConfig, MCPTool, SamplingOptions, SamplingRequest
};

pub mod builtin;
pub mod schema;
pub mod server;
pub mod server_utils;
pub mod types;
pub mod utils;

// This file now serves as a re-export hub for backward compatibility
// All implementation details have been moved to separate modules
