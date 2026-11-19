//! Provider-neutral request and response types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A tool call the model asked for.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // "function"
    pub function: ToolCallFunction,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

/// Extended reasoning effort preset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    #[default]
    Off,
    Low,
    Medium,
    High,
    Auto,
}

impl ThinkingEffort {
    /// The preset with this stored name, if any.
    pub fn parse(effort: &str) -> Option<Self> {
        match effort {
            "off" => Some(Self::Off),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Who sent a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Tool,
}

/// One part of a message.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentPart {
    Text(String),
    /// `data` is base64; `None` when only a reference to the file was kept.
    Image {
        data: Option<String>,
        mime_type: String,
    },
    /// `data` is base64; `None` when only a reference to the file was kept.
    Audio {
        data: Option<String>,
        mime_type: String,
    },
    /// Content a provider cannot receive, by kind (for example `resource`).
    Unsupported(String),
}

/// One turn of the conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: Vec<ContentPart>,
    /// Calls an assistant turn made.
    pub tool_calls: Vec<ToolCall>,
    /// The call a tool turn answers.
    pub tool_call_id: Option<String>,
    /// Reasoning an assistant turn produced.
    pub thinking: Option<String>,
    /// The provider's signature over that turn's reasoning (Anthropic thinking
    /// signature, Gemini thought signature), sent back with it.
    pub thinking_signature: Option<String>,
}

/// A tool the model may call.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema of the arguments.
    pub parameters: Value,
}

/// One chat call.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub system_prompt: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: Option<u32>,
    /// Sent only when set; otherwise the provider's default applies.
    pub temperature: Option<f64>,
    pub thinking_effort: ThinkingEffort,
}

/// Token counts, in the shape the frontend stores.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<UsageDetails>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_to_first_token: Option<f64>,
}

/// One fragment of a streamed tool call. Fragments with the same `index`
/// belong to one call: the first carries `id` and `name`, later ones carry
/// more `arguments` text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub function: ToolCallFunctionDelta,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolCallFunctionDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A piece of the JSON arguments text; may be empty.
    pub arguments: String,
}

/// What one stream line added to the answer.
///
/// Provider-neutral and free of UI types, so the GUI and the TUI read the same
/// events. The JSON shape matches the chunk shape the frontend stream parser
/// reads (`content`, `thinking`, `thinkingSignature`, `usage`, `tool_calls`).
/// Every field is a change, not a running total, except `usage` and
/// `thinkingSignature`, which replace the earlier value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(default, rename = "tool_calls", skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallDelta>,
    /// Why the answer ended, in the OpenAI words (`stop`, `tool_calls`, `length`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

impl StreamDelta {
    /// True when the line added nothing worth sending.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// What one completed call produced.
#[derive(Debug, Default, PartialEq)]
pub struct CompletedChat {
    pub content: String,
    pub thinking: String,
    /// See [`ChatMessage::thinking_signature`].
    pub thinking_signature: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
    pub finish_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usage_serializes_like_the_frontend_and_omits_what_is_unknown() {
        let bare = Usage {
            prompt_tokens: 12,
            completion_tokens: 3,
            total_tokens: 15,
            ..Usage::default()
        };
        assert_eq!(
            serde_json::to_value(&bare).unwrap(),
            json!({ "promptTokens": 12, "completionTokens": 3, "totalTokens": 15 })
        );

        let full = Usage {
            cached_prompt_tokens: Some(8),
            details: Some(UsageDetails {
                reasoning_tokens: Some(2),
                eval_duration: Some(125.5),
                prompt_eval_duration: Some(40.0),
                time_to_first_token: Some(40.0),
            }),
            ..bare
        };
        let value = serde_json::to_value(&full).unwrap();
        assert_eq!(value["cachedPromptTokens"], 8);
        assert_eq!(value["details"]["reasoningTokens"], 2);
        assert_eq!(value["details"]["evalDuration"], 125.5);
        assert_eq!(value["details"]["promptEvalDuration"], 40.0);
        assert_eq!(value["details"]["timeToFirstToken"], 40.0);

        let timing_only = UsageDetails {
            eval_duration: Some(12.0),
            ..UsageDetails::default()
        };
        assert_eq!(
            serde_json::to_value(&timing_only).unwrap(),
            json!({ "evalDuration": 12.0 })
        );
    }

    #[test]
    fn effort_parses_its_lowercase_names_only() {
        assert_eq!(ThinkingEffort::parse("auto"), Some(ThinkingEffort::Auto));
        assert_eq!(ThinkingEffort::parse("High"), None);
        assert_eq!(ThinkingEffort::default(), ThinkingEffort::Off);
    }
}
