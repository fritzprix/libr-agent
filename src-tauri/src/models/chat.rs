use crate::agent::types::ToolCall;
use crate::mcp::types::MCPContent;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const MESSAGE_SOURCE_CHANNEL: &str = "channel";
pub(crate) const MESSAGE_SOURCE_COMPACT_SUMMARY: &str = "compact-summary";
pub(crate) const MESSAGE_SOURCE_COMPACTION_INSTRUCTION: &str = "compaction-instruction";
pub(crate) const MESSAGE_SOURCE_RECOVERY: &str = "recovery";
pub(crate) const MESSAGE_SOURCE_SCHEDULED_TASK: &str = "scheduled_task";
pub(crate) const MESSAGE_SOURCE_TOOL: &str = "tool";
pub(crate) const MESSAGE_SOURCE_UI: &str = "ui";

const COMPACT_SUMMARY_ID_PREFIX: &str = "compact-summary-";
const COMPACTION_INSTRUCTION_ID_PREFIX: &str = "compaction-instruction-";

/// Default timestamp generator for serde deserialization fallback
fn default_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnownMessageSource {
    Channel,
    CompactSummary,
    CompactionInstruction,
    Recovery,
    ScheduledTask,
    Tool,
    Ui,
}

impl KnownMessageSource {
    fn from_source(source: &str) -> Option<Self> {
        match source {
            MESSAGE_SOURCE_CHANNEL => Some(Self::Channel),
            MESSAGE_SOURCE_COMPACT_SUMMARY => Some(Self::CompactSummary),
            MESSAGE_SOURCE_COMPACTION_INSTRUCTION => Some(Self::CompactionInstruction),
            MESSAGE_SOURCE_RECOVERY => Some(Self::Recovery),
            MESSAGE_SOURCE_SCHEDULED_TASK => Some(Self::ScheduledTask),
            MESSAGE_SOURCE_TOOL => Some(Self::Tool),
            MESSAGE_SOURCE_UI => Some(Self::Ui),
            _ => None,
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        if id.starts_with(COMPACT_SUMMARY_ID_PREFIX) {
            return Some(Self::CompactSummary);
        }

        if id.starts_with(COMPACTION_INSTRUCTION_ID_PREFIX) {
            return Some(Self::CompactionInstruction);
        }

        None
    }
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
    fn known_source(&self) -> Option<KnownMessageSource> {
        self.source
            .as_deref()
            .and_then(KnownMessageSource::from_source)
            .or_else(|| KnownMessageSource::from_id(&self.id))
    }

    pub fn is_compact_summary(&self) -> bool {
        matches!(
            self.known_source(),
            Some(KnownMessageSource::CompactSummary)
        )
    }

    pub fn is_compaction_instruction(&self) -> bool {
        matches!(
            self.known_source(),
            Some(KnownMessageSource::CompactionInstruction)
        )
    }

    pub fn is_recovery_message(&self) -> bool {
        matches!(self.known_source(), Some(KnownMessageSource::Recovery))
    }

    pub fn is_internal_synthetic_user_message(&self) -> bool {
        self.role == "user"
            && matches!(
                self.known_source(),
                Some(KnownMessageSource::CompactionInstruction | KnownMessageSource::Recovery)
            )
    }

    pub fn is_external_request_message(&self) -> bool {
        self.role == "user" && !self.is_internal_synthetic_user_message()
    }
}
