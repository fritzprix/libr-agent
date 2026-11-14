//! Turns LibrAgent messages and tools into a provider-neutral `llm_client` request.
//!
//! The wire format itself lives in the `llm-client` crate. What stays here is
//! what only LibrAgent knows: compact summaries ride in the system prompt, and
//! a UI-sourced message counts as the user's.

use crate::mcp::types::{MCPContent, MCPTool};
use crate::models::chat::{Message, MessageSource};
use llm_client::{ChatMessage, ChatRequest, ContentPart, Role, ThinkingEffort, ToolSpec};
use serde_json::{json, Value};

/// Text parts of a message, joined by newlines. Non-text parts are skipped.
pub(super) fn text_of(content: &[MCPContent]) -> String {
    content
        .iter()
        .filter_map(|item| match item {
            MCPContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_compact_summary(message: &Message) -> bool {
    matches!(message.source, Some(MessageSource::CompactSummary))
        || message.id.starts_with("compact-summary-")
}

fn content_part(item: &MCPContent) -> ContentPart {
    match item {
        MCPContent::Text { text } => ContentPart::Text(text.clone()),
        MCPContent::Image {
            data, mime_type, ..
        } => ContentPart::Image {
            data: data.clone(),
            mime_type: mime_type.clone(),
        },
        MCPContent::Audio {
            data, mime_type, ..
        } => ContentPart::Audio {
            data: data.clone(),
            mime_type: mime_type.clone(),
        },
        MCPContent::Resource { .. } => ContentPart::Unsupported("resource".to_string()),
        MCPContent::Thinking { .. } => ContentPart::Unsupported("thinking".to_string()),
        MCPContent::ToolCall { .. } => ContentPart::Unsupported("tool_call".to_string()),
    }
}

fn chat_message(message: &Message) -> Option<ChatMessage> {
    let role = if matches!(message.source, Some(MessageSource::Ui)) {
        Role::User
    } else {
        match message.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            other => {
                log::warn!(
                    "Message {} has unsupported role '{other}'; skipped",
                    message.id
                );
                return None;
            }
        }
    };
    Some(ChatMessage {
        role,
        content: message.content.iter().map(content_part).collect(),
        tool_calls: message.tool_calls.clone().unwrap_or_default(),
        tool_call_id: message.tool_call_id.clone(),
        thinking: message.thinking.clone(),
        thinking_signature: message.thinking_signature.clone(),
    })
}

fn tool_spec(tool: &MCPTool) -> ToolSpec {
    ToolSpec {
        name: tool.name.clone(),
        description: tool.description.clone(),
        parameters: serde_json::to_value(&tool.input_schema).unwrap_or_else(|_| json!({})),
    }
}

/// Generation settings the GUI applies to every provider.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct GenerationOptions {
    pub(super) temperature: Option<f64>,
    pub(super) thinking_effort: ThinkingEffort,
}

/// The temperature to send: only when the user turned the override on, kept
/// in the 0–2 range the Settings screen allows.
pub(super) fn temperature_override(enabled: Option<&Value>, value: Option<&Value>) -> Option<f64> {
    if enabled.and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let temperature = value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(DEFAULT_TEMPERATURE);
    Some(temperature.clamp(0.0, 2.0))
}

/// The Settings default when the override is on but no value was stored.
const DEFAULT_TEMPERATURE: f64 = 0.7;

pub(super) fn chat_request(
    model: &str,
    system_prompt: Option<&str>,
    messages: &[Message],
    tools: &[MCPTool],
    max_tokens: Option<u32>,
    options: GenerationOptions,
) -> ChatRequest {
    // Compact summaries ride in the system message, as in the frontend.
    let mut system_parts: Vec<String> = Vec::new();
    if let Some(prompt) = system_prompt.filter(|prompt| !prompt.is_empty()) {
        system_parts.push(prompt.to_string());
    }
    system_parts.extend(
        messages
            .iter()
            .filter(|message| is_compact_summary(message))
            .map(|message| text_of(&message.content))
            .filter(|text| !text.is_empty()),
    );

    ChatRequest {
        model: model.to_string(),
        system_prompt: (!system_parts.is_empty()).then(|| system_parts.join("\n\n")),
        messages: messages
            .iter()
            .filter(|message| !is_compact_summary(message))
            .filter_map(chat_message)
            .collect(),
        tools: tools.iter().map(tool_spec).collect(),
        max_tokens,
        temperature: options.temperature,
        thinking_effort: options.thinking_effort,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(value: Value) -> Message {
        serde_json::from_value(value).expect("message fixture")
    }

    fn request(system_prompt: Option<&str>, messages: &[Message]) -> ChatRequest {
        chat_request(
            "m",
            system_prompt,
            messages,
            &[],
            None,
            GenerationOptions::default(),
        )
    }

    #[test]
    fn system_prompt_and_compact_summaries_become_one_system_prompt() {
        let messages = vec![
            message(json!({
                "id": "compact-summary-1", "sessionId": "s", "role": "user",
                "content": [{ "type": "text", "text": "Earlier: the user asked for X." }],
                "source": "compact-summary",
            })),
            message(json!({
                "id": "u1", "sessionId": "s", "role": "user",
                "content": [{ "type": "text", "text": "continue" }],
            })),
        ];

        let request = request(Some("You are an agent."), &messages);

        assert_eq!(
            request.system_prompt.as_deref(),
            Some("You are an agent.\n\nEarlier: the user asked for X.")
        );
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, Role::User);
        assert_eq!(
            request.messages[0].content,
            vec![ContentPart::Text("continue".to_string())]
        );
    }

    #[test]
    fn no_prompt_and_no_summary_means_no_system_prompt() {
        assert_eq!(request(Some(""), &[]).system_prompt, None);
    }

    #[test]
    fn a_ui_sourced_message_is_the_users_and_unknown_roles_are_dropped() {
        let messages = vec![
            message(json!({
                "id": "m1", "sessionId": "s", "role": "assistant", "source": "ui",
                "content": [{ "type": "text", "text": "typed by the user" }],
            })),
            message(json!({
                "id": "m2", "sessionId": "s", "role": "system",
                "content": [{ "type": "text", "text": "ignored" }],
            })),
        ];

        let request = request(None, &messages);

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, Role::User);
    }

    #[test]
    fn tool_calls_thinking_media_and_tools_carry_over() {
        let messages = vec![
            message(json!({
                "id": "a1", "sessionId": "s", "role": "assistant", "content": [],
                "toolCalls": [{
                    "id": "call_1", "type": "function",
                    "function": { "name": "media__captureScreen", "arguments": "{}" },
                }],
                "thinking": "need a screenshot",
                "thinkingSignature": "sig-1",
            })),
            message(json!({
                "id": "t1", "sessionId": "s", "role": "tool", "toolCallId": "call_1",
                "content": [
                    { "type": "image", "data": "QUJD", "mimeType": "image/png" },
                    { "type": "thinking", "thinking": "not sendable" },
                ],
            })),
        ];
        let tool: MCPTool = serde_json::from_value(json!({
            "name": "workspace__readFile",
            "description": "Read a file",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } } },
        }))
        .expect("tool fixture");

        let request = chat_request(
            "m",
            None,
            &messages,
            &[tool],
            Some(4096),
            GenerationOptions {
                temperature: Some(0.3),
                thinking_effort: ThinkingEffort::High,
            },
        );

        let assistant = &request.messages[0];
        assert_eq!(
            assistant.tool_calls[0].function.name,
            "media__captureScreen"
        );
        assert_eq!(assistant.thinking.as_deref(), Some("need a screenshot"));
        assert_eq!(assistant.thinking_signature.as_deref(), Some("sig-1"));
        let tool_turn = &request.messages[1];
        assert_eq!(tool_turn.tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(
            tool_turn.content[0],
            ContentPart::Image {
                data: Some("QUJD".to_string()),
                mime_type: "image/png".to_string(),
            }
        );
        assert_eq!(
            tool_turn.content[1],
            ContentPart::Unsupported("thinking".to_string())
        );
        assert_eq!(request.tools[0].name, "workspace__readFile");
        assert_eq!(
            request.tools[0].parameters["properties"]["path"]["type"],
            "string"
        );
        assert_eq!(request.max_tokens, Some(4096));
        assert_eq!(request.temperature, Some(0.3));
        assert_eq!(request.thinking_effort, ThinkingEffort::High);
    }

    #[test]
    fn temperature_is_sent_only_with_the_override_on_and_in_range() {
        let on = json!(true);
        assert_eq!(temperature_override(None, Some(&json!(0.4))), None);
        assert_eq!(
            temperature_override(Some(&json!(false)), Some(&json!(0.4))),
            None
        );
        assert_eq!(
            temperature_override(Some(&on), Some(&json!(0.4))),
            Some(0.4)
        );
        assert_eq!(temperature_override(Some(&on), Some(&json!(9))), Some(2.0));
        assert_eq!(
            temperature_override(Some(&on), Some(&json!("hot"))),
            Some(0.7)
        );
        assert_eq!(temperature_override(Some(&on), None), Some(0.7));
    }
}
