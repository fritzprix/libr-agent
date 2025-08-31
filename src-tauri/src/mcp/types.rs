use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    #[serde(default = "default_transport")]
    pub transport: String, // "stdio" | "http" | "websocket"
    pub url: Option<String>,
    pub port: Option<u16>,
}

fn default_transport() -> String {
    "stdio".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(flatten)]
    pub additional: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: crate::mcp::schema::JSONSchema,
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<crate::mcp::schema::JSONSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<MCPToolAnnotations>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingOptions {
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub stop_sequences: Option<Vec<String>>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingRequest {
    pub prompt: String,
    pub options: Option<SamplingOptions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MCPResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<MCPError>,
}

impl MCPResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

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

pub struct MCPConnection {
    pub client: rmcp::service::RunningService<rmcp::service::RoleClient, ()>,
}
