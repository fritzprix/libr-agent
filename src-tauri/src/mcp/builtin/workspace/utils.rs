use crate::mcp::MCPResponse;
use serde_json::{json, Value};

use crate::session_isolation::IsolationLevel;
use std::process::Output;
use tracing::info;

pub mod constants {
    pub const DEFAULT_EXECUTION_TIMEOUT: u64 = 30;
    pub const MAX_EXECUTION_TIMEOUT: u64 = 300;
    pub const MAX_CODE_SIZE: usize = 1024 * 1024; // 1MB
    pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB

    // Terminal-related constants
    pub const MAX_TERMINAL_BUFFER_SIZE: usize = 10 * 1024 * 1024; // 10MB per terminal
    pub const MAX_TERMINAL_BUFFER_LINES: usize = 10_000;
    pub const MAX_ACTIVE_TERMINALS: usize = 20;
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

/// Parse the isolation level from arguments, providing a default
pub fn parse_isolation_level(level_val: Option<&Value>) -> IsolationLevel {
    level_val
        .and_then(|v| v.as_str())
        .map(|level| match level {
            "basic" => IsolationLevel::Basic,
            "high" => IsolationLevel::High,
            _ => IsolationLevel::Medium,
        })
        .unwrap_or(IsolationLevel::Medium)
}

/// Format the output of a synchronous execution into an MCPResponse
pub fn format_sync_execution_output(
    request_id: Value,
    output: &Output,
    language: &str,
    session_id: &str,
) -> MCPResponse {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let success = output.status.success();
    let exit_code = output.status.code().unwrap_or(-1);

    let result_text = if success {
        if stdout.trim().is_empty() && stderr.trim().is_empty() {
            format!("{language} code executed successfully (no output)")
        } else if stderr.trim().is_empty() {
            format!("Output:\n{}", stdout.trim())
        } else {
            format!(
                "Output:\n{}\n\nWarnings/Errors:\n{}",
                stdout.trim(),
                stderr.trim()
            )
        }
    } else {
        format!(
            "Execution failed (exit code: {}):\nSTDOUT:\n{}\n\nSTDERR:\n{}",
            exit_code,
            stdout.trim(),
            stderr.trim()
        )
    };

    info!(
        "Isolated sync {} execution completed. Session: {}, Success: {}, Output length: {}",
        language,
        session_id,
        success,
        result_text.len()
    );

    create_success_response(request_id, &result_text)
}
