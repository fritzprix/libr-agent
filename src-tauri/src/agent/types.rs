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

/// DTO for Message received from TypeScript frontend
/// Matches src/models/chat.ts Message interface
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageDto {
    pub id: String,
    pub session_id: String, // TS: sessionId
    pub role: String,

    /// TS: MCPContent[]
    /// We receive it as a JSON Value (Array) and will stringify it for DB
    pub content: serde_json::Value,

    /// TS: ToolCall[]
    /// We receive as proper objects, will stringify for DB
    pub tool_calls: Option<Vec<ToolCall>>,

    pub tool_call_id: Option<String>,
    pub is_streaming: Option<bool>,
    pub thinking: Option<String>,
    pub thinking_signature: Option<String>,
    pub assistant_id: Option<String>,

    /// TS: AttachmentReference[]
    pub attachments: Option<serde_json::Value>,

    /// TS: { id, name, input }
    pub tool_use: Option<serde_json::Value>,

    // TS: Date (string in JSON) or number?
    // Typescript Date serializes to ISO string usually, but `createdAt: i64` in DB.
    // Let's check how TS sends it.
    // In LLMServiceContext: `createdAt: new Date()` -> JSON string "2024-..."
    // BUT Message struct expects `i64`.
    // We should accept typical JS Date serialization which is String or Number.
    // Actually, `messages_commands.rs` `Message` has `created_at: i64`.
    // If TS sends ISO string, Serde will fail if we define it as i64.
    // We should use `Option<serde_json::Value>` or custom deserializer.
    // Safe bet: accepts anything for now?
    // Or better, check what TS sends.
    // `LLMServiceContext.tsx`: `createdAt: new Date()`
    // `JSON.stringify(new Date())` is "2024-12-..." (String).
    // So if DTO has `created_at: i64`, it will fail.
    // We need `created_at: String` or `created_at: serde_json::Value` in DTO, then parse to i64.
    pub created_at: Option<serde_json::Value>,
    pub updated_at: Option<serde_json::Value>,

    pub source: Option<String>,
    pub error: Option<serde_json::Value>, // TS has complex error object
}

impl AgentMessageDto {
    pub fn into_message(self) -> Message {
        // Parse dates
        let created_at = parse_date_to_timestamp(self.created_at)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
        let updated_at = parse_date_to_timestamp(self.updated_at).unwrap_or(created_at);

        Message {
            id: self.id,
            session_id: self.session_id,
            role: self.role,
            content: self.content.to_string(), // Convert JSON array to string
            tool_calls: self
                .tool_calls
                .map(|tc| serde_json::to_string(&tc).unwrap_or_default()),
            tool_call_id: self.tool_call_id,
            is_streaming: self.is_streaming,
            thinking: self.thinking,
            thinking_signature: self.thinking_signature,
            assistant_id: self.assistant_id,
            attachments: self.attachments.map(|a| a.to_string()),
            tool_use: self.tool_use.map(|t| t.to_string()),
            created_at,
            updated_at,
            source: self.source,
            error: self.error.map(|e| e.to_string()),
        }
    }
}

fn parse_date_to_timestamp(val: Option<serde_json::Value>) -> Option<i64> {
    match val {
        Some(serde_json::Value::Number(n)) => n.as_i64(),
        Some(serde_json::Value::String(s)) => {
            // Try parsing ISO string
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
                Some(dt.timestamp_millis())
            } else {
                None
            }
        }
        _ => None,
    }
}
