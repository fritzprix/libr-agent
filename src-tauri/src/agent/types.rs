use crate::commands::messages_commands::Message;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // "function"
    pub function: ToolCallFunction,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

/// Explicit content type for MCP messages
/// Matches MCP spec (text, image, resource)
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MCPContent {
    Text {
        text: String,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource {
        uri: String,
        #[serde(rename = "mimeType")]
        mime_type: Option<String>,
        text: Option<String>,
        blob: Option<String>,
    },
}

/// AgentMessageDto is now deprecated - use Message directly.
/// Message now has structured types matching TypeScript,
/// with JSON conversion handled only in Repository layer.
/// This type alias maintains compatibility during migration.
pub type AgentMessageDto = Message;
