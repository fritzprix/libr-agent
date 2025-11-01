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
    Http {
        url: String,
        #[serde(default = "default_protocol_version")]
        protocol_version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
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

/// V2 MCP Server Configuration (MCP 2025-06-18 Spec Compliant)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfigV2 {
    pub name: String,
    pub transport: TransportConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<OAuthConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ServerMetadata>,
}

// ========================================
// Legacy Type Definition (Backward Compatibility)
// ========================================

/// Legacy MCP server configuration (stdio-only)
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

// ========================================
// Auto-conversion Wrapper
// ========================================

/// Wrapper for automatic detection and conversion between V1 and V2 configs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MCPServerConfigWrapper {
    V2(Box<MCPServerConfigV2>), // Boxed to reduce enum size
    Legacy(MCPServerConfig),
}

/// Automatic conversion from Legacy to V2
impl From<MCPServerConfig> for MCPServerConfigV2 {
    fn from(legacy: MCPServerConfig) -> Self {
        let transport =
            if let (Some(command), transport_type) = (&legacy.command, &legacy.transport) {
                if transport_type == "stdio" {
                    TransportConfig::Stdio {
                        command: command.clone(),
                        args: legacy.args.unwrap_or_default(),
                        env: legacy.env.unwrap_or_default(),
                    }
                } else if let Some(url) = legacy.url {
                    // If transport is http but formatted as legacy config
                    TransportConfig::Http {
                        url,
                        protocol_version: default_protocol_version(),
                        session_id: None,
                        headers: None,
                        enable_sse: None,
                        security: None,
                    }
                } else {
                    // Fallback to stdio
                    TransportConfig::Stdio {
                        command: command.clone(),
                        args: legacy.args.unwrap_or_default(),
                        env: legacy.env.unwrap_or_default(),
                    }
                }
            } else {
                // No command specified, assume HTTP if URL is present
                if let Some(url) = legacy.url {
                    TransportConfig::Http {
                        url,
                        protocol_version: default_protocol_version(),
                        session_id: None,
                        headers: None,
                        enable_sse: None,
                        security: None,
                    }
                } else {
                    // Cannot create valid transport without command or URL
                    // This is an error case, but we'll create a dummy stdio transport
                    TransportConfig::Stdio {
                        command: "echo".to_string(),
                        args: vec!["Error: Invalid legacy config".to_string()],
                        env: HashMap::new(),
                    }
                }
            };

        MCPServerConfigV2 {
            name: legacy.name,
            transport,
            authentication: None,
            metadata: None,
        }
    }
}

/// Automatic conversion from Wrapper to V2 (with unboxing)
impl From<MCPServerConfigWrapper> for MCPServerConfigV2 {
    fn from(wrapper: MCPServerConfigWrapper) -> Self {
        match wrapper {
            MCPServerConfigWrapper::V2(boxed) => *boxed,
            MCPServerConfigWrapper::Legacy(legacy) => legacy.into(),
        }
    }
}

/// Convert wrapper to Legacy
impl From<MCPServerConfigWrapper> for MCPServerConfig {
    fn from(wrapper: MCPServerConfigWrapper) -> Self {
        match wrapper {
            MCPServerConfigWrapper::V2(boxed) => (*boxed).into(),
            MCPServerConfigWrapper::Legacy(legacy) => legacy,
        }
    }
}

/// Temporary conversion from V2 back to Legacy (for backward compatibility)
/// This is needed until MCPServerManager is updated to handle V2 configs directly
impl From<MCPServerConfigV2> for MCPServerConfig {
    fn from(v2: MCPServerConfigV2) -> Self {
        match v2.transport {
            TransportConfig::Stdio { command, args, env } => MCPServerConfig {
                name: v2.name,
                command: Some(command),
                args: Some(args),
                env: Some(env),
                transport: "stdio".to_string(),
                url: None,
                port: None,
            },
            TransportConfig::Http { url, .. } => MCPServerConfig {
                name: v2.name,
                command: None,
                args: None,
                env: None,
                transport: "http".to_string(),
                url: Some(url),
                port: None,
            },
        }
    }
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
// Unit Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_stdio_to_v2_conversion() {
        let legacy = MCPServerConfig {
            name: "test-server".to_string(),
            command: Some("node".to_string()),
            args: Some(vec!["server.js".to_string()]),
            env: Some(HashMap::from([(
                "API_KEY".to_string(),
                "secret".to_string(),
            )])),
            transport: "stdio".to_string(),
            url: None,
            port: None,
        };

        let v2: MCPServerConfigV2 = legacy.into();

        assert_eq!(v2.name, "test-server");
        match v2.transport {
            TransportConfig::Stdio { command, args, env } => {
                assert_eq!(command, "node");
                assert_eq!(args, vec!["server.js"]);
                assert_eq!(env.get("API_KEY"), Some(&"secret".to_string()));
            }
            _ => panic!("Expected Stdio transport"),
        }
        assert!(v2.authentication.is_none());
        assert!(v2.metadata.is_none());
    }

    #[test]
    fn test_legacy_http_to_v2_conversion() {
        let legacy = MCPServerConfig {
            name: "http-server".to_string(),
            command: None,
            args: None,
            env: None,
            transport: "http".to_string(),
            url: Some("https://api.example.com/mcp".to_string()),
            port: Some(8080),
        };

        let v2: MCPServerConfigV2 = legacy.into();

        assert_eq!(v2.name, "http-server");
        match v2.transport {
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
    fn test_wrapper_deserialize_v2_stdio() {
        let json = r#"{
            "name": "stdio-server",
            "transport": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-example"],
                "env": {}
            }
        }"#;

        let wrapper: MCPServerConfigWrapper = serde_json::from_str(json).unwrap();
        let v2: MCPServerConfigV2 = wrapper.into();

        assert_eq!(v2.name, "stdio-server");
        match v2.transport {
            TransportConfig::Stdio { command, .. } => {
                assert_eq!(command, "npx");
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_wrapper_deserialize_v2_http() {
        let json = r#"{
            "name": "http-server",
            "transport": {
                "type": "http",
                "url": "https://api.example.com/mcp",
                "protocol_version": "2025-06-18"
            }
        }"#;

        let wrapper: MCPServerConfigWrapper = serde_json::from_str(json).unwrap();
        let v2: MCPServerConfigV2 = wrapper.into();

        assert_eq!(v2.name, "http-server");
        match v2.transport {
            TransportConfig::Http { url, .. } => {
                assert_eq!(url, "https://api.example.com/mcp");
            }
            _ => panic!("Expected Http transport"),
        }
    }

    #[test]
    fn test_wrapper_deserialize_legacy() {
        let json = r#"{
            "name": "legacy-server",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-example"],
            "env": {},
            "transport": "stdio"
        }"#;

        let wrapper: MCPServerConfigWrapper = serde_json::from_str(json).unwrap();
        let v2: MCPServerConfigV2 = wrapper.into();

        assert_eq!(v2.name, "legacy-server");
        match v2.transport {
            TransportConfig::Stdio { command, .. } => {
                assert_eq!(command, "npx");
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_v2_with_oauth_serialization() {
        let config = MCPServerConfigV2 {
            name: "oauth-server".to_string(),
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
        let deserialized: MCPServerConfigV2 = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "oauth-server");
        assert!(deserialized.authentication.is_some());
        assert!(deserialized.metadata.is_some());
    }
}
