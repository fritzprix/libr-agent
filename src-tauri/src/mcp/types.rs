use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ========================================
// V2 Type Definitions (MCP 2025-06-18 Spec)
// ========================================

/// Represents transport-specific configuration using discriminated union pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransportConfig {
    /// Standard I/O transport for local MCP servers
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    /// HTTP/HTTPS transport for remote MCP servers (Streamable HTTP)
    /// `http-sse` is accepted as an alias for backward compatibility with frontend-saved records
    #[serde(alias = "http-sse")]
    Http {
        url: String,
        #[serde(default = "default_protocol_version")]
        #[serde(rename = "protocolVersion", alias = "protocol_version")]
        protocol_version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "sessionId", alias = "session_id")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "enableSSE", alias = "enable_sse")]
        enable_sse: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        security: Option<SecurityConfig>,
    },
}

fn default_protocol_version() -> String {
    "2025-06-18".to_string()
}

/// Security configuration for HTTP transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub enable_dns_rebinding_protection: bool,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

/// OAuth 2.1 authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    #[serde(rename = "type")]
    pub oauth_type: String, // Always "oauth2.1"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_url: Option<String>, // RFC 8414 discovery endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_endpoint: Option<String>, // RFC 7591
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(default = "default_use_pkce")]
    pub use_pkce: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_parameter: Option<String>, // RFC 9728
}

fn default_use_pkce() -> bool {
    true
}

/// Server metadata (vendor, version, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// MCP Server Configuration (MCP 2025-06-18 Spec Compliant)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    /// Server name - optional in JSON, will be populated from DB name column if missing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub transport: TransportConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<OAuthConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ServerMetadata>,
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

/// Represents service information for content origin tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub server_name: String,
    pub tool_name: String,
    pub backend_type: String, // "ExternalMCP" | "BuiltInWeb" | "BuiltInRust"
}

/// Represents MCP content items (text or resource).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum MCPContent {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "isError")]
        is_error: Option<bool>,
    },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "audio")]
    Audio {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "resource")]
    Resource {
        resource: serde_json::Value,
        #[serde(rename = "serviceInfo")]
        service_info: ServiceInfo,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(rename = "thinkingTime")]
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking_time: Option<f64>,
    },
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
}

/// Represents the pure result of a tool execution (without JSON-RPC wrapper).
/// This is what built-in tools should return before being wrapped in MCPResponse.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPResult {
    /// Content items returned by the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<MCPContent>>,
    /// Structured data returned by the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Value>,
    /// Flag indicating if this is a tool execution error (not a protocol error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl MCPResult {
    /// Creates a successful MCPResult with text content.
    #[allow(dead_code)]
    pub fn success(text: &str) -> Self {
        Self {
            content: Some(vec![MCPContent::Text {
                text: text.to_string(),
                is_error: None,
            }]),
            structured_content: None,
            is_error: Some(false),
        }
    }

    /// Creates a successful MCPResult with text and structured content.
    #[allow(dead_code)]
    pub fn success_with_data(text: &str, data: serde_json::Value) -> Self {
        Self {
            content: Some(vec![MCPContent::Text {
                text: text.to_string(),
                is_error: None,
            }]),
            structured_content: Some(data),
            is_error: Some(false),
        }
    }

    /// Creates a non-error informational MCPResult.
    #[allow(dead_code)]
    pub fn informational(message: &str) -> Self {
        Self::success(message)
    }

    /// Creates a non-error informational MCPResult with structured data.
    #[allow(dead_code)]
    pub fn informational_with_data(message: &str, data: serde_json::Value) -> Self {
        Self::success_with_data(message, data)
    }

    /// Creates an error MCPResult.
    #[allow(dead_code)]
    pub fn error(message: &str) -> Self {
        Self {
            content: Some(vec![MCPContent::Text {
                text: message.to_string(),
                is_error: Some(true),
            }]),
            structured_content: None,
            is_error: Some(true),
        }
    }

    /// Creates an error MCPResult with additional structured data.
    #[allow(dead_code)]
    pub fn error_with_data(message: &str, data: serde_json::Value) -> Self {
        Self {
            content: Some(vec![MCPContent::Text {
                text: message.to_string(),
                is_error: Some(true),
            }]),
            structured_content: Some(serde_json::json!({
                "error": data
            })),
            is_error: Some(true),
        }
    }
}

/// JSON-RPC 2.0 request/response identifier
/// According to JSON-RPC 2.0 spec, id can be a String, Number, or null
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum JsonRpcId {
    String(String),
    Number(i64),
    Null,
}

/// Represents a standard MCP response, compliant with JSON-RPC 2.0.
#[derive(Debug, Serialize, Deserialize)]
pub struct MCPResponse {
    /// The JSON-RPC version string.
    pub jsonrpc: String,
    /// The request identifier (can be String, Number, or null per JSON-RPC 2.0 spec)
    pub id: Option<JsonRpcId>,
    /// The result of the operation, if successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<MCPResponseResult>,
    /// The error object, if an error occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<MCPError>,
}

/// Union type for all possible MCP response results based on the method called.
/// This ensures type safety while supporting the polymorphic nature of JSON-RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MCPResponseResult {
    /// Result from tools/call - tool execution result
    ToolCall(MCPResult),
    /// Result from tools/list - list of available tools
    ToolsList { tools: Vec<MCPTool> },
    /// Result from resources/list - list of available resources
    ResourcesList { resources: Vec<MCPResource> },
    /// Result from prompts/list - list of available prompts
    PromptsList { prompts: Vec<MCPPrompt> },
    /// Result from initialize - server capabilities
    Initialize {
        #[serde(rename = "protocolVersion")]
        protocol_version: String,
        capabilities: ServerCapabilities,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "serverInfo")]
        server_info: Option<ServerInfo>,
    },
    /// Generic fallback for other operations
    Generic(serde_json::Value),
}

/// Server capabilities as defined in MCP protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<serde_json::Value>,
}

/// Server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// MCP Resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// MCP Prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPPrompt {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Prompt argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

impl MCPResponse {
    /// Creates a successful `MCPResponse` from a tool call result.
    pub fn success(id: JsonRpcId, result: MCPResult) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(MCPResponseResult::ToolCall(result)),
            error: None,
        }
    }

    /// Creates a successful `MCPResponse` with generic result.
    pub fn success_generic(id: JsonRpcId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(MCPResponseResult::Generic(result)),
            error: None,
        }
    }

    /// Creates an error `MCPResponse`.
    pub fn error(id: JsonRpcId, code: i32, message: &str) -> Self {
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
    /// The server configuration including transport type.
    pub config: MCPServerConfig,
}

/// Options for service context operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceContextOptions {
    /// The session ID for context isolation.
    #[serde(rename = "sessionId", alias = "session_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The assistant ID for context filtering.
    #[serde(rename = "assistantId", alias = "assistant_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_id: Option<String>,
}

/// Represents the service context with structured state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceContext<T = serde_json::Value> {
    /// The context prompt describing the current state.
    pub context_prompt: String,
    /// Optional structured state data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_state: Option<T>,
}

// ========================================
// Builtin Server Metadata (UI-facing)
// ========================================

/// UI metadata for builtin MCP servers (matches TypeScript ServiceMetadata interface)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinServerMetadata {
    /// Human-friendly display name for the UI
    pub display_name: String,
    /// Description of what the server does
    pub description: String,
    /// Optional icon identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Complete information about a builtin server including metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinServerInfo {
    /// Server identifier (e.g., "workspace", "attachments")
    pub name: String,
    /// UI metadata
    pub metadata: BuiltinServerMetadata,
    /// Number of tools this server provides
    pub tool_count: usize,
}

// ========================================
// Unit Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_stdio_serialization() {
        let config = MCPServerConfig {
            name: Some("stdio-server".to_string()),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-example".to_string(),
                ],
                env: HashMap::new(),
            },
            authentication: None,
            metadata: None,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: MCPServerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name.as_deref(), Some("stdio-server"));
        match deserialized.transport {
            TransportConfig::Stdio { command, .. } => {
                assert_eq!(command, "npx");
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_mcp_config_http_serialization() {
        let config = MCPServerConfig {
            name: Some("http-server".to_string()),
            transport: TransportConfig::Http {
                url: "https://api.example.com/mcp".to_string(),
                protocol_version: "2025-06-18".to_string(),
                session_id: None,
                headers: None,
                enable_sse: Some(false),
                security: None,
            },
            authentication: None,
            metadata: None,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: MCPServerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name.as_deref(), Some("http-server"));
        match deserialized.transport {
            TransportConfig::Http {
                url,
                protocol_version,
                ..
            } => {
                assert_eq!(url, "https://api.example.com/mcp");
                assert_eq!(protocol_version, "2025-06-18");
            }
            _ => panic!("Expected Http transport"),
        }
    }

    #[test]
    fn test_mcp_config_with_oauth_serialization() {
        let config = MCPServerConfig {
            name: Some("oauth-server".to_string()),
            transport: TransportConfig::Http {
                url: "https://api.example.com/mcp".to_string(),
                protocol_version: "2025-06-18".to_string(),
                session_id: None,
                headers: None,
                enable_sse: Some(false),
                security: None,
            },
            authentication: Some(OAuthConfig {
                oauth_type: "oauth2.1".to_string(),
                discovery_url: Some(
                    "https://auth.example.com/.well-known/oauth-authorization-server".to_string(),
                ),
                authorization_endpoint: None,
                token_endpoint: None,
                registration_endpoint: None,
                client_id: Some("test-client".to_string()),
                redirect_uri: Some("libr-agent://oauth/callback".to_string()),
                scopes: Some(vec!["read".to_string(), "write".to_string()]),
                use_pkce: true,
                resource_parameter: None,
            }),
            metadata: Some(ServerMetadata {
                description: Some("Test server with OAuth".to_string()),
                vendor: Some("Example Corp".to_string()),
                version: Some("1.0.0".to_string()),
            }),
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: MCPServerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name.as_deref(), Some("oauth-server"));
        assert!(deserialized.authentication.is_some());
        assert!(deserialized.metadata.is_some());
    }

    /// Regression: frontend saves transport type as "http-sse", Rust must accept it via alias.
    /// Previously caused a panic in queries.rs when listing external servers.
    #[test]
    fn test_http_sse_alias_deserializes_as_http() {
        let json = r#"{"type":"http-sse","url":"https://mcp.exa.ai/mcp?exaApiKey=test","protocol_version":"2025-06-18"}"#;
        let transport: TransportConfig = serde_json::from_str(json).unwrap();
        match transport {
            TransportConfig::Http { url, .. } => {
                assert_eq!(url, "https://mcp.exa.ai/mcp?exaApiKey=test");
            }
            _ => panic!("Expected Http transport for 'http-sse' type tag"),
        }
    }

    /// Regression: MCPServerConfig with "http-sse" type must not panic during deserialization.
    #[test]
    fn test_mcp_config_http_sse_roundtrip() {
        let raw = r#"{"transport":{"type":"http-sse","url":"https://api.example.com/mcp","protocol_version":"2025-06-18"}}"#;
        let config: MCPServerConfig = serde_json::from_str(raw).unwrap();
        match config.transport {
            TransportConfig::Http { url, .. } => assert_eq!(url, "https://api.example.com/mcp"),
            _ => panic!("Expected Http transport"),
        }
    }
}
