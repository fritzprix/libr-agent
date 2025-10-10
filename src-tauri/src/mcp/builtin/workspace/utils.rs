use crate::mcp::MCPResponse;
use serde_json::{json, Value};

pub mod constants {
    pub const DEFAULT_EXECUTION_TIMEOUT: u64 = 30;
    pub const MAX_EXECUTION_TIMEOUT: u64 = 300;

    // Default max file size (10MB) used if environment variable is not set or invalid.
    const DEFAULT_MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB

    /// Returns the configured maximum file size in bytes.
    ///
    /// Lookup order:
    /// 1. Environment variable `SYNAPTICFLOW_MAX_FILE_SIZE` (interpreted as bytes, integer)
    /// 2. Fallback to `DEFAULT_MAX_FILE_SIZE`.
    pub fn max_file_size() -> usize {
        use once_cell::sync::Lazy;

        static MAX_SIZE: Lazy<usize> = Lazy::new(|| {
            // Try to load from .env first (non-fatal)
            let _ = dotenvy::dotenv();

            std::env::var("SYNAPTICFLOW_MAX_FILE_SIZE")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(DEFAULT_MAX_FILE_SIZE)
        });

        *MAX_SIZE
    }
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
