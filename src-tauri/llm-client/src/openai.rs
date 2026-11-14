//! The OpenAI chat-completions wire format.
//!
//! Mirrors the frontend converter (`src/lib/ai-service/openai/message-converter.ts`):
//! user / assistant / tool turns, tool calls, reasoning, images, and audio.

use crate::types::{ChatMessage, ChatRequest, ContentPart, Role, ToolSpec};
use serde_json::{json, Value};

fn text_of(content: &[ContentPart]) -> String {
    content
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_media(part: &ContentPart) -> bool {
    matches!(part, ContentPart::Image { .. } | ContentPart::Audio { .. })
}

/// One content part in the OpenAI multimodal format.
fn content_part(part: &ContentPart) -> Value {
    match part {
        ContentPart::Text(text) => json!({ "type": "text", "text": text }),
        ContentPart::Image {
            data: Some(data),
            mime_type,
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
        ContentPart::Audio {
            data: Some(data),
            mime_type,
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
        ContentPart::Image { data: None, .. } => {
            json!({ "type": "text", "text": "[Image not available]" })
        }
        ContentPart::Audio { data: None, .. } => {
            json!({ "type": "text", "text": "[Audio not available]" })
        }
        ContentPart::Unsupported(kind) => {
            json!({ "type": "text", "text": format!("[Unsupported content: {kind}]") })
        }
    }
}

/// A plain string when every part is text, otherwise the multimodal array.
fn user_content(content: &[ContentPart]) -> Value {
    if content
        .iter()
        .all(|part| matches!(part, ContentPart::Text(_)))
    {
        Value::String(text_of(content))
    } else {
        Value::Array(content.iter().map(content_part).collect())
    }
}

/// Name of the tool whose call produced `tool_call_id`, searching backwards.
fn tool_name_for<'a>(earlier: &'a [ChatMessage], tool_call_id: &str) -> Option<&'a str> {
    earlier.iter().rev().find_map(|message| {
        message
            .tool_calls
            .iter()
            .find(|call| call.id == tool_call_id)
            .map(|call| call.function.name.as_str())
    })
}

pub(crate) fn chat_messages(system_prompt: Option<&str>, messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = Vec::with_capacity(messages.len() + 1);
    if let Some(prompt) = system_prompt.filter(|prompt| !prompt.is_empty()) {
        out.push(json!({ "role": "system", "content": prompt }));
    }

    for (index, message) in messages.iter().enumerate() {
        match message.role {
            Role::User => out.push(json!({
                "role": "user",
                "content": user_content(&message.content),
            })),
            Role::Assistant => {
                let text = text_of(&message.content);
                let mut assistant = json!({ "role": "assistant" });
                // `null` content is only valid next to tool calls.
                assistant["content"] = if text.is_empty() && !message.tool_calls.is_empty() {
                    Value::Null
                } else {
                    Value::String(text)
                };
                if !message.tool_calls.is_empty() {
                    assistant["tool_calls"] = json!(message.tool_calls);
                }
                if let Some(thinking) = message.thinking.as_ref().filter(|t| !t.is_empty()) {
                    assistant["reasoning_content"] = json!(thinking);
                }
                out.push(assistant);
            }
            Role::Tool => {
                let Some(tool_call_id) = message.tool_call_id.as_deref() else {
                    log::warn!("A tool message has no tool_call_id; skipped");
                    continue;
                };
                let media: Vec<&ContentPart> =
                    message.content.iter().filter(|p| is_media(p)).collect();
                let mut parts: Vec<String> = Vec::new();
                let text = text_of(&message.content);
                if !text.is_empty() {
                    parts.push(text);
                }
                let images = media
                    .iter()
                    .filter(|part| matches!(part, ContentPart::Image { .. }))
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
        }
    }

    out
}

pub(crate) fn chat_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            let mut parameters = tool.parameters.clone();
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

/// The streaming chat-completions body for `request`, sent as `model`.
pub(crate) fn request_body(model: &str, request: &ChatRequest) -> Value {
    let mut body = json!({
        "model": model,
        "messages": chat_messages(request.system_prompt.as_deref(), &request.messages),
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if let Some(max_tokens) = request.max_tokens {
        body["max_completion_tokens"] = json!(max_tokens);
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    if !request.tools.is_empty() {
        body["tools"] = Value::Array(chat_tools(&request.tools));
        body["tool_choice"] = json!("auto");
    }
    body
}

/// JSON with object keys sorted, so equal values give equal text whatever
/// the key order.
pub(crate) fn stable_stringify(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(stable_stringify).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            let entries: Vec<String> = entries
                .into_iter()
                .map(|(key, value)| {
                    format!("{}:{}", Value::from(key.as_str()), stable_stringify(value))
                })
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        other => other.to_string(),
    }
}

/// 32-bit FNV-1a over UTF-16 code units, in hex: the frontend's
/// `stableHashKeyPart`.
pub(crate) fn hash_key_part(text: &str) -> String {
    let mut hash: u32 = 2_166_136_261;
    for unit in text.encode_utf16() {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(16_777_619);
    }
    format!("{hash:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ToolCall, ToolCallFunction};

    fn message(role: Role, content: Vec<ContentPart>) -> ChatMessage {
        ChatMessage {
            role,
            content,
            tool_calls: Vec::new(),
            tool_call_id: None,
            thinking: None,
        }
    }

    fn text(value: &str) -> ContentPart {
        ContentPart::Text(value.to_string())
    }

    fn call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: "{}".to_string(),
            },
        }
    }

    #[test]
    fn the_system_prompt_leads_and_user_text_is_a_plain_string() {
        let out = chat_messages(
            Some("You are an agent."),
            &[message(Role::User, vec![text("continue")])],
        );
        assert_eq!(
            out,
            vec![
                json!({ "role": "system", "content": "You are an agent." }),
                json!({ "role": "user", "content": "continue" }),
            ]
        );
        assert_eq!(chat_messages(Some(""), &[]), Vec::<Value>::new());
    }

    #[test]
    fn an_assistant_tool_call_and_its_result_keep_their_pairing() {
        let mut assistant = message(Role::Assistant, Vec::new());
        assistant.tool_calls = vec![call("call_1", "workspace__readFile")];
        assistant.thinking = Some("need the file".to_string());
        let mut tool = message(Role::Tool, vec![text("file body")]);
        tool.tool_call_id = Some("call_1".to_string());

        let out = chat_messages(None, &[assistant, tool]);

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
        let mut assistant = message(Role::Assistant, Vec::new());
        assistant.tool_calls = vec![call("call_9", "media__captureScreen")];
        let mut tool = message(
            Role::Tool,
            vec![
                text("captured"),
                ContentPart::Image {
                    data: Some("QUJD".to_string()),
                    mime_type: "image/png".to_string(),
                },
            ],
        );
        tool.tool_call_id = Some("call_9".to_string());

        let out = chat_messages(None, &[assistant, tool]);

        assert_eq!(out[1]["content"], "captured\n\n[Image output: 1 file(s)]");
        assert_eq!(out[2]["role"], "user");
        let note = out[2]["content"][0]["text"].as_str().unwrap();
        assert!(note.contains("media__captureScreen"), "{note}");
        assert_eq!(
            out[2]["content"][1]["image_url"]["url"],
            "data:image/png;base64,QUJD"
        );
    }

    #[test]
    fn user_media_and_unsupported_parts_use_the_multimodal_array() {
        let out = chat_messages(
            None,
            &[message(
                Role::User,
                vec![
                    text("listen"),
                    ContentPart::Audio {
                        data: Some("UklG".to_string()),
                        mime_type: "audio/wav".to_string(),
                    },
                    ContentPart::Image {
                        data: None,
                        mime_type: "image/png".to_string(),
                    },
                    ContentPart::Unsupported("resource".to_string()),
                ],
            )],
        );
        let parts = out[0]["content"].as_array().unwrap();
        assert_eq!(parts[1]["input_audio"]["format"], "wav");
        assert_eq!(parts[2]["text"], "[Image not available]");
        assert_eq!(parts[3]["text"], "[Unsupported content: resource]");
    }

    #[test]
    fn a_tool_message_without_its_call_id_is_dropped() {
        let out = chat_messages(None, &[message(Role::Tool, vec![text("orphan")])]);
        assert!(out.is_empty());
    }

    #[test]
    fn the_body_adds_optional_fields_only_when_given() {
        let bare = request_body("m", &ChatRequest::default());
        assert_eq!(bare["stream"], true);
        assert_eq!(bare["stream_options"]["include_usage"], true);
        for key in [
            "tools",
            "tool_choice",
            "max_completion_tokens",
            "temperature",
        ] {
            assert!(bare.get(key).is_none(), "{key} must be absent");
        }

        let request = ChatRequest {
            tools: vec![ToolSpec {
                name: "workspace__readFile".to_string(),
                description: "Read a file".to_string(),
                parameters: json!({ "properties": { "path": { "type": "string" } } }),
            }],
            max_tokens: Some(4096),
            temperature: Some(0.2),
            ..ChatRequest::default()
        };
        let full = request_body("m", &request);
        assert_eq!(full["max_completion_tokens"], 4096);
        assert_eq!(full["temperature"], 0.2);
        assert_eq!(full["tool_choice"], "auto");
        assert_eq!(full["tools"][0]["function"]["name"], "workspace__readFile");
        // A schema without a type gets one.
        assert_eq!(full["tools"][0]["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn stable_text_ignores_key_order() {
        let left = json!({ "b": [1, { "y": 2, "x": "s" }], "a": null });
        let right = json!({ "a": null, "b": [1, { "x": "s", "y": 2 }] });
        assert_eq!(stable_stringify(&left), stable_stringify(&right));
        assert_eq!(
            stable_stringify(&left),
            r#"{"a":null,"b":[1,{"x":"s","y":2}]}"#
        );
    }

    #[test]
    fn hash_key_part_matches_the_frontend_fnv1a() {
        // Values from `stableHashKeyPart` in base-service-utils.ts.
        assert_eq!(hash_key_part(""), "811c9dc5");
        assert_eq!(hash_key_part("a"), "e40c292c");
    }
}
