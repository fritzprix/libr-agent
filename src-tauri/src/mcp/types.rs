use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the configuration for an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    /// The unique name of the server.
    pub name: String,
    /// The command to execute to start the server (for stdio transport).
    pub command: Option<String>,
    /// An array of arguments to pass to the command.
    pub args: Option<Vec<String>>,
    /// Environment variables to set for the server process.
    pub env: Option<HashMap<String, String>>,
    /// The transport protocol ("stdio", "http", "websocket"). Defaults to "stdio".
    #[serde(default = "default_transport")]
    pub transport: String,
    /// The URL of the server (for http or websocket transports).
    pub url: Option<String>,
    /// The port number of the server (for http or websocket transports).
    pub port: Option<u16>,
}

/// Provides the default value for the `transport` field.
fn default_transport() -> String {
    "stdio".to_string()
}

/// Represents metadata annotations for an `MCPTool`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolAnnotations {
    /// The intended audience for the tool's output (e.g., "user", "assistant").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<String>>,
    /// A priority level for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
    /// An ISO 8601 timestamp of when the tool was last modified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    /// A map for any other custom annotations.
    #[serde(flatten)]
    pub additional: serde_json::Map<String, serde_json::Value>,
}

/// Represents a tool that can be invoked via the Model-Context-Protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    /// The unique name of the tool.
    pub name: String,
    /// A human-readable title for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A detailed description of what the tool does.
    pub description: String,
    /// The JSON Schema for the tool's input parameters.
    #[serde(rename = "inputSchema")]
    pub input_schema: crate::mcp::schema::JSONSchema,
    /// The JSON Schema for the tool's output.
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<crate::mcp::schema::JSONSchema>,
    /// Additional metadata about the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<MCPToolAnnotations>,
}

/// Represents a JSON-RPC error object as defined by the MCP specification.
#[derive(Debug, Serialize, Deserialize)]
pub struct MCPError {
    /// A number that indicates the error type that occurred.
    pub code: i32,
    /// A string providing a short description of the error.
    pub message: String,
    /// A primitive or structured value that contains additional information about the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Defines options for text generation (sampling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingOptions {
    /// The model to use for the generation.
    pub model: Option<String>,
    /// The maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// The sampling temperature.
    pub temperature: Option<f64>,
    /// The nucleus sampling probability.
    pub top_p: Option<f64>,
    /// The number of top tokens to consider for sampling.
    pub top_k: Option<u32>,
    /// A list of sequences to stop generation at.
    pub stop_sequences: Option<Vec<String>>,
    /// The presence penalty.
    pub presence_penalty: Option<f64>,
    /// The frequency penalty.
    pub frequency_penalty: Option<f64>,
}

/// Represents a request for text generation (sampling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingRequest {
    /// The prompt to use for generation.
    pub prompt: String,
    /// Optional parameters for the sampling request.
    pub options: Option<SamplingOptions>,
}

/// Represents a standard MCP response, compliant with JSON-RPC 2.0.
#[derive(Debug, Serialize, Deserialize)]
pub struct MCPResponse {
    /// The JSON-RPC version string.
    pub jsonrpc: String,
    /// The request identifier.
    pub id: Option<serde_json::Value>,
    /// The result of the operation, if successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// The error object, if an error occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<MCPError>,
}

impl MCPResponse {
    /// Creates a successful `MCPResponse`.
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error `MCPResponse`.
    pub fn error(id: serde_json::Value, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: None,
            error: Some(MCPError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }
}

/// Represents an active connection to an external MCP server.
#[derive(Debug)]
pub struct MCPConnection {
    /// The `rmcp` client instance for communicating with the server.
    pub client: rmcp::service::RunningService<rmcp::service::RoleClient, ()>,
}
