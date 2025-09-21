use crate::mcp::MCPResponse;
use serde_json::{json, Value};

/// Generate a new request ID for MCP responses
pub fn generate_request_id() -> Value {
    Value::String(cuid2::create_id())
}

/// Create a dual response with both text content and structured data
pub fn create_dual_response(
    request_id: Value,
    message: &str,
    structured_content: Value,
) -> MCPResponse {
    MCPResponse::success(
        request_id,
        json!({
            "content": [{
                "type": "text",
                "text": message
            }],
            "structuredContent": structured_content
        }),
    )
}

/// Create an error response with consistent formatting
pub fn create_error_response(request_id: Value, code: i32, message: &str) -> MCPResponse {
    MCPResponse::error(request_id, code, message)
}
