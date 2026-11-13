//! Turns a LibrAgent completion request into an OpenAI chat-completions body.
//!
//! Mirrors the frontend converter (`src/lib/ai-service/openai/message-converter.ts`)
//! for the parts a WebView-free session needs: system prompt with compact
//! summaries, user / assistant / tool turns, tool calls, images, and audio.

use crate::mcp::types::{MCPContent, MCPTool};
use crate::models::chat::{Message, MessageSource};
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

fn is_media(item: &MCPContent) -> bool {
    matches!(item, MCPContent::Image { .. } | MCPContent::Audio { .. })
}

/// One content part in the OpenAI multimodal format.
fn content_part(item: &MCPContent) -> Value {
    match item {
        MCPContent::Text { text } => json!({ "type": "text", "text": text }),
        MCPContent::Image {
            data: Some(data),
            mime_type,
            ..
        } => {
            let mime = if mime_type.is_empty() {
                "image/jpeg"
            } else {
                mime_type
            };
            json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{mime};base64,{data}") },
            })
        }
        MCPContent::Audio {
            data: Some(data),
            mime_type,
            ..
        } => {
            let format = if mime_type.contains("wav") {
                "wav"
            } else {
                "mp3"
            };
            json!({
                "type": "input_audio",
                "input_audio": { "data": data, "format": format },
            })
        }
        MCPContent::Image { .. } => json!({ "type": "text", "text": "[Image not available]" }),
        MCPContent::Audio { .. } => json!({ "type": "text", "text": "[Audio not available]" }),
        MCPContent::Resource { .. } => {
            json!({ "type": "text", "text": "[Unsupported content: resource]" })
        }
        MCPContent::Thinking { .. } => {
            json!({ "type": "text", "text": "[Unsupported content: thinking]" })
        }
        MCPContent::ToolCall { .. } => {
            json!({ "type": "text", "text": "[Unsupported content: tool_call]" })
        }
    }
}

/// A plain string when every part is text, otherwise the multimodal array.
fn user_content(content: &[MCPContent]) -> Value {
    if content
        .iter()
        .all(|item| matches!(item, MCPContent::Text { .. }))
    {
        Value::String(text_of(content))
    } else {
        Value::Array(content.iter().map(content_part).collect())
    }
}

/// Name of the tool whose call produced `tool_call_id`, searching backwards.
fn tool_name_for<'a>(earlier: &'a [Message], tool_call_id: &str) -> Option<&'a str> {
    earlier.iter().rev().find_map(|message| {
        message
            .tool_calls
            .as_ref()?
            .iter()
            .find(|call| call.id == tool_call_id)
            .map(|call| call.function.name.as_str())
    })
}

pub(super) fn chat_messages(system_prompt: Option<&str>, messages: &[Message]) -> Vec<Value> {
    let mut out = Vec::with_capacity(messages.len() + 1);

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
    if !system_parts.is_empty() {
        out.push(json!({ "role": "system", "content": system_parts.join("\n\n") }));
    }

    for (index, message) in messages.iter().enumerate() {
        if is_compact_summary(message) {
            continue;
        }
        let role = if matches!(message.source, Some(MessageSource::Ui)) {
            "user"
        } else {
            message.role.as_str()
        };

        match role {
            "user" => out.push(json!({
                "role": "user",
                "content": user_content(&message.content),
            })),
            "assistant" => {
                let text = text_of(&message.content);
                let tool_calls = message
                    .tool_calls
                    .as_ref()
                    .filter(|calls| !calls.is_empty());
                let mut assistant = json!({ "role": "assistant" });
                // `null` content is only valid next to tool calls.
                assistant["content"] = if text.is_empty() && tool_calls.is_some() {
                    Value::Null
                } else {
                    Value::String(text)
                };
                if let Some(calls) = tool_calls {
                    assistant["tool_calls"] = json!(calls);
                }
                if let Some(thinking) = message.thinking.as_ref().filter(|t| !t.is_empty()) {
                    assistant["reasoning_content"] = json!(thinking);
                }
                out.push(assistant);
            }
            "tool" => {
                let Some(tool_call_id) = message.tool_call_id.as_deref() else {
                    log::warn!("Tool message {} has no tool_call_id; skipped", message.id);
                    continue;
                };
                let media: Vec<&MCPContent> =
                    message.content.iter().filter(|i| is_media(i)).collect();
                let mut parts: Vec<String> = Vec::new();
                let text = text_of(&message.content);
                if !text.is_empty() {
                    parts.push(text);
                }
                let images = media
                    .iter()
                    .filter(|item| matches!(item, MCPContent::Image { .. }))
                    .count();
                let audios = media.len() - images;
                if images > 0 {
                    parts.push(format!("[Image output: {images} file(s)]"));
                }
                if audios > 0 {
                    parts.push(format!("[Audio output: {audios} file(s)]"));
                }
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": parts.join("\n\n"),
                }));

                // Tool messages cannot carry media, so it follows as a user turn
                // that says where it came from.
                if !media.is_empty() {
                    let origin = match tool_name_for(&messages[..index], tool_call_id) {
                        Some(name) => format!("tool \"{name}\" (ID: {tool_call_id})"),
                        None => format!("tool (ID: {tool_call_id})"),
                    };
                    let mut content = vec![json!({
                        "type": "text",
                        "text": format!(
                            "[Image/Audio output from {origin}. This is the result of your own tool execution, not a user request.]"
                        ),
                    })];
                    content.extend(media.into_iter().map(content_part));
                    out.push(json!({ "role": "user", "content": content }));
                }
            }
            other => log::warn!(
                "Message {} has unsupported role '{other}'; skipped",
                message.id
            ),
        }
    }

    out
}

pub(super) fn chat_tools(tools: &[MCPTool]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            let mut parameters = serde_json::to_value(&tool.input_schema).unwrap_or(json!({}));
            // Providers reject a parameters schema without a type.
            if let Some(object) = parameters.as_object_mut() {
                object.entry("type").or_insert(json!("object"));
            } else {
                parameters = json!({ "type": "object", "properties": {} });
            }
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": parameters,
                },
            })
        })
        .collect()
}

/// The streaming chat-completions request body.
pub(super) fn chat_request_body(
    model: &str,
    system_prompt: Option<&str>,
    messages: &[Message],
    tools: &[MCPTool],
    max_tokens: Option<u32>,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": chat_messages(system_prompt, messages),
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if let Some(max_tokens) = max_tokens {
        body["max_completion_tokens"] = json!(max_tokens);
    }
    if !tools.is_empty() {
        body["tools"] = Value::Array(chat_tools(tools));
        body["tool_choice"] = json!("auto");
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(value: Value) -> Message {
        serde_json::from_value(value).expect("message fixture")
    }

    #[test]
    fn system_prompt_and_compact_summaries_become_one_system_message() {
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

        let out = chat_messages(Some("You are an agent."), &messages);

        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0]["role"], "system");
        assert_eq!(
            out[0]["content"],
            "You are an agent.\n\nEarlier: the user asked for X."
        );
        assert_eq!(out[1], json!({ "role": "user", "content": "continue" }));
    }

    #[test]
    fn an_assistant_tool_call_and_its_result_keep_their_pairing() {
        let messages = vec![
            message(json!({
                "id": "a1", "sessionId": "s", "role": "assistant", "content": [],
                "toolCalls": [{
                    "id": "call_1", "type": "function",
                    "function": { "name": "workspace__readFile", "arguments": "{\"path\":\"a.txt\"}" },
                }],
                "thinking": "need the file",
            })),
            message(json!({
                "id": "t1", "sessionId": "s", "role": "tool", "toolCallId": "call_1",
                "content": [{ "type": "text", "text": "file body" }],
            })),
        ];

        let out = chat_messages(None, &messages);

        assert_eq!(out[0]["role"], "assistant");
        assert_eq!(out[0]["content"], Value::Null);
        assert_eq!(out[0]["tool_calls"][0]["id"], "call_1");
        assert_eq!(
            out[0]["tool_calls"][0]["function"]["name"],
            "workspace__readFile"
        );
        assert_eq!(out[0]["reasoning_content"], "need the file");
        assert_eq!(
            out[1],
            json!({ "role": "tool", "tool_call_id": "call_1", "content": "file body" })
        );
    }

    #[test]
    fn tool_media_follows_as_a_user_turn_that_names_the_tool() {
        let messages = vec![
            message(json!({
                "id": "a1", "sessionId": "s", "role": "assistant", "content": [],
                "toolCalls": [{
                    "id": "call_9", "type": "function",
                    "function": { "name": "media__captureScreen", "arguments": "{}" },
                }],
            })),
            message(json!({
                "id": "t1", "sessionId": "s", "role": "tool", "toolCallId": "call_9",
                "content": [
                    { "type": "text", "text": "captured" },
                    { "type": "image", "data": "QUJD", "mimeType": "image/png" },
                ],
            })),
        ];

        let out = chat_messages(None, &messages);

        assert_eq!(out[1]["content"], "captured\n\n[Image output: 1 file(s)]");
        assert_eq!(out[2]["role"], "user");
        let text = out[2]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("media__captureScreen"), "{text}");
        assert_eq!(
            out[2]["content"][1]["image_url"]["url"],
            "data:image/png;base64,QUJD"
        );
    }

    #[test]
    fn a_ui_sourced_message_is_sent_as_a_user_turn() {
        let messages = vec![message(json!({
            "id": "m1", "sessionId": "s", "role": "assistant", "source": "ui",
            "content": [{ "type": "text", "text": "typed by the user" }],
        }))];

        let out = chat_messages(None, &messages);

        assert_eq!(
            out,
            vec![json!({ "role": "user", "content": "typed by the user" })]
        );
    }

    #[test]
    fn the_body_adds_tools_and_token_limit_only_when_given() {
        let bare = chat_request_body("m", None, &[], &[], None);
        assert_eq!(bare["stream"], true);
        assert_eq!(bare["stream_options"]["include_usage"], true);
        assert!(bare.get("tools").is_none());
        assert!(bare.get("tool_choice").is_none());
        assert!(bare.get("max_completion_tokens").is_none());

        let tool: MCPTool = serde_json::from_value(json!({
            "name": "workspace__readFile",
            "description": "Read a file",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } } },
        }))
        .expect("tool fixture");
        let full = chat_request_body("m", None, &[], &[tool], Some(4096));
        assert_eq!(full["max_completion_tokens"], 4096);
        assert_eq!(full["tool_choice"], "auto");
        assert_eq!(full["tools"][0]["type"], "function");
        assert_eq!(full["tools"][0]["function"]["name"], "workspace__readFile");
        assert_eq!(full["tools"][0]["function"]["parameters"]["type"], "object");
    }
}
