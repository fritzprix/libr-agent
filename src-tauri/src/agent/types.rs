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

/// AgentMessageDto is now deprecated - use Message directly.
/// Message now has structured types matching TypeScript,
/// with JSON conversion handled only in Repository layer.
/// This type alias maintains compatibility during migration.
#[deprecated(note = "Use Message directly.")]
pub type AgentMessageDto = Message;
