use crate::mcp::MCPResponse;
use serde_json::{json, Value};

pub mod constants {
    pub const DEFAULT_EXECUTION_TIMEOUT: u64 = 30;
    pub const MAX_EXECUTION_TIMEOUT: u64 = 300;
    pub const MAX_CODE_SIZE: usize = 1024 * 1024; // 1MB
    pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB
}

/// Generate a new request ID for MCP responses
pub fn generate_request_id() -> Value {
    Value::String(cuid2::create_id())
}

/// Create a success response with text content
pub fn create_success_response(request_id: Value, message: &str) -> MCPResponse {
    MCPResponse::success(
        request_id,
        json!({
            "content": [{
                "type": "text",
                "text": message
            }]
        }),
    )
}

/// Create an error response with consistent formatting
pub fn create_error_response(request_id: Value, code: i32, message: &str) -> MCPResponse {
    MCPResponse::error(request_id, code, message)
}

/// Validate timeout value, applying default and max limits
pub fn validate_timeout(timeout: Option<u64>) -> u64 {
    timeout
        .unwrap_or(constants::DEFAULT_EXECUTION_TIMEOUT)
        .min(constants::MAX_EXECUTION_TIMEOUT)
}
