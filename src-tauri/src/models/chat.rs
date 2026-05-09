use crate::agent::types::ToolCall;
use crate::mcp::types::MCPContent;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::{SystemTime, UNIX_EPOCH};

const COMPACT_SUMMARY_ID_PREFIX: &str = "compact-summary-";
const COMPACTION_INSTRUCTION_ID_PREFIX: &str = "compaction-instruction-";

/// Default timestamp generator for serde deserialization fallback
fn default_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MessageSource {
    Assistant,
    Api,
    AgentTool,
    SwarmLegacy,
    Channel,
    CompactSummary,
    CompactionInstruction,
    Recovery,
    ScheduledTask,
    Tool,
    Ui,
    Unknown(String),
}

impl MessageSource {
    pub fn from_raw(source: impl Into<String>) -> Self {
        let source = source.into();
        match source.as_str() {
            "assistant" => Self::Assistant,
            "api" => Self::Api,
            "agent_tool" => Self::AgentTool,
            "swarm_legacy" => Self::SwarmLegacy,
            "channel" => Self::Channel,
            "compact-summary" => Self::CompactSummary,
            "compaction-instruction" => Self::CompactionInstruction,
            "recovery" => Self::Recovery,
            "scheduled_task" => Self::ScheduledTask,
            "tool" => Self::Tool,
            "ui" => Self::Ui,
            _ => Self::Unknown(source),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Assistant => "assistant",
            Self::Api => "api",
            Self::AgentTool => "agent_tool",
            Self::SwarmLegacy => "swarm_legacy",
            Self::Channel => "channel",
            Self::CompactSummary => "compact-summary",
            Self::CompactionInstruction => "compaction-instruction",
            Self::Recovery => "recovery",
            Self::ScheduledTask => "scheduled_task",
            Self::Tool => "tool",
            Self::Ui => "ui",
            Self::Unknown(source) => source.as_str(),
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

    fn is_internal_synthetic_user_source(&self) -> bool {
        matches!(self, Self::CompactionInstruction | Self::Recovery)
    }

    fn is_external_request_source(&self) -> bool {
        matches!(
            self,
            Self::Api | Self::SwarmLegacy | Self::Channel | Self::ScheduledTask
        )
    }
}

impl Serialize for MessageSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MessageSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let source = String::deserialize(deserializer)?;
        Ok(Self::from_raw(source))
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
    pub source: Option<MessageSource>,
    /// Error information as structured value
    pub error: Option<serde_json::Value>,
    /// Optional metadata for tool execution tracking (mirrors frontend Message.metadata)
    pub metadata: Option<serde_json::Value>,
}

impl Message {
    pub fn new_user_message(
        session_id: String,
        text: String,
        source: Option<MessageSource>,
        assistant_id: Option<String>,
    ) -> Self {
        let now = default_timestamp();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            role: "user".to_string(),
            content: vec![MCPContent::Text {
                text,
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id,
            attachments: None,
            tool_use: None,
            usage: None,
            created_at: now,
            updated_at: now,
            source,
            error: None,
            metadata: None,
        }
    }

    pub fn new_compact_summary_message(session_id: &str, text: String, created_at: i64) -> Self {
        Self {
            id: format!("compact-summary-{}", session_id),
            session_id: session_id.to_string(),
            role: "assistant".to_string(),
            content: vec![MCPContent::Text {
                text,
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            created_at,
            updated_at: created_at,
            source: Some(MessageSource::CompactSummary),
            error: None,
            metadata: None,
        }
    }

    fn source_with_legacy_fallback(&self) -> Option<MessageSource> {
        match self.source.as_ref() {
            Some(MessageSource::Unknown(_)) | None => MessageSource::from_id(&self.id),
            Some(source) => Some(source.clone()),
        }
    }

    pub fn is_compact_summary(&self) -> bool {
        matches!(
            self.source_with_legacy_fallback(),
            Some(MessageSource::CompactSummary)
        )
    }

    pub fn is_compaction_instruction(&self) -> bool {
        matches!(
            self.source_with_legacy_fallback(),
            Some(MessageSource::CompactionInstruction)
        )
    }

    pub fn is_recovery_message(&self) -> bool {
        matches!(
            self.source_with_legacy_fallback(),
            Some(MessageSource::Recovery)
        )
    }

    pub fn is_internal_synthetic_user_message(&self) -> bool {
        self.role == "user"
            && self
                .source_with_legacy_fallback()
                .is_some_and(|source| source.is_internal_synthetic_user_source())
    }

    pub fn is_external_request_message(&self) -> bool {
        self.role == "user"
            && !self.is_internal_synthetic_user_message()
            && self
                .source_with_legacy_fallback()
                .is_none_or(|source| source.is_external_request_source())
    }
}
