use crate::agent::types::ToolCall;
use crate::mcp::types::MCPContent;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default timestamp generator for serde deserialization fallback
fn default_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Message data model matching the frontend TypeScript Message interface.
/// Stores chat messages for sessions with support for various content types.
/// All fields use structured types - JSON serialization handled in Repository layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    /// Structured content array (MCPContent[]) - matches TypeScript
    pub content: Vec<MCPContent>,
    /// Tool calls as structured array - matches TypeScript
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub is_streaming: Option<bool>,
    pub thinking: Option<String>,
    pub thinking_signature: Option<String>,
    pub assistant_id: Option<String>,
    /// Attachments as structured value
    pub attachments: Option<serde_json::Value>,
    /// Tool use as structured value
    pub tool_use: Option<serde_json::Value>,
    /// Token usage metrics
    pub usage: Option<serde_json::Value>,
    #[serde(default = "default_timestamp")]
    pub created_at: i64, // Unix timestamp in milliseconds
    #[serde(default = "default_timestamp")]
    pub updated_at: i64, // Unix timestamp in milliseconds
    pub source: Option<String>,
    /// Error information as structured value
    pub error: Option<serde_json::Value>,
    /// Optional metadata for tool execution tracking (mirrors frontend Message.metadata)
    pub metadata: Option<serde_json::Value>,
}

impl Message {
    pub fn is_compact_summary(&self) -> bool {
        self.source.as_deref() == Some("compact-summary") || self.id.starts_with("compact-summary-")
    }

    pub fn is_compaction_instruction(&self) -> bool {
        self.source.as_deref() == Some("compaction-instruction")
            || self.id.starts_with("compaction-instruction-")
    }
}
