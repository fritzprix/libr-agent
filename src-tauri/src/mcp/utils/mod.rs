pub mod command_helper;
pub mod env;
pub mod schema_builder;
pub mod server_name;

pub use server_name::{
    is_valid_mcp_server_name, sanitize_mcp_server_name, unique_session_server_name,
};

/// Serialize a list of `rmcp::model::Tool` (raw MCP protocol type) to a compact JSON cache string.
/// Used by `test_server_connection` in the builtin tool server operations.
pub fn serialize_rmcp_tools(tools: &[rmcp::model::Tool]) -> String {
    let arr: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description.as_deref().unwrap_or("")
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

/// Serialize a list of `crate::mcp::types::MCPTool` (our internal type) to a compact JSON cache string.
/// Used by `probe_mcp_server` in `mcp_commands.rs`.
pub fn serialize_mcp_tools(tools: &[crate::mcp::types::MCPTool]) -> String {
    let arr: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

// Tests moved to tests/mcp_utils_tests.rs (integration tests).
// Run: cargo test --tests mcp_utils
