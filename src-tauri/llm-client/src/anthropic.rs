//! Anthropic: the Messages API (`/v1/messages`).
//!
//! Mirrors the frontend's `AnthropicService` and its converter
//! (`src/lib/ai-service/anthropic/`): the system prompt, the tool list, and the
//! last message (plus the middle of a long history) are prompt-cache
//! breakpoints; tool results ride in one user turn; reasoning goes back as a
//! signed thinking block.

use crate::openai::{object_schema, text_of};
use crate::provider::{ChatProvider, Endpoint};
use crate::stream::StreamParser;
use crate::types::{
    ChatMessage, ChatRequest, CompletedChat, ContentPart, Role, ThinkingEffort, ToolCall,
    ToolCallFunction, Usage,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const API_VERSION: &str = "2023-06-01";
/// The GUI's default output limit. Anthropic requires one on every call.
const DEFAULT_MAX_TOKENS: u32 = 8192;
/// The smallest thinking budget Anthropic accepts.
const MIN_THINKING_BUDGET: u32 = 1024;
/// A history this long also gets a cache breakpoint at its middle.
const MIN_MESSAGES_FOR_MIDPOINT_BREAKPOINT: usize = 8;

#[derive(Debug, Clone)]
pub struct Anthropic {
    pub endpoint: Endpoint,
}

impl ChatProvider for Anthropic {
    fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn request_url(&self, _model: &str) -> String {
        format!("{}/messages", self.endpoint.base_url.trim_end_matches('/'))
    }

    fn auth_header(&self, api_key: &str) -> (&'static str, String) {
        ("x-api-key", api_key.to_string())
    }

    fn extra_headers(&self) -> &[(&'static str, &'static str)] {
        &[("anthropic-version", API_VERSION)]
    }

    fn build_body(&self, request: &ChatRequest) -> Value {
        request_body(&self.model_id(&request.model), request)
    }

    fn stream_parser(&self) -> Box<dyn StreamParser> {
        Box::<AnthropicStream>::default()
    }
}

fn ephemeral() -> Value {
    json!({ "type": "ephemeral" })
}

/// The thinking budget for `effort`, kept below `max_tokens` as Anthropic
/// requires. `None` when thinking is off or `max_tokens` leaves no room.
fn thinking_budget(effort: ThinkingEffort, max_tokens: u32) -> Option<u32> {
    let preset = match effort {
        ThinkingEffort::Off => return None,
        ThinkingEffort::Low => 1024,
        ThinkingEffort::Medium | ThinkingEffort::Auto => 8192,
        ThinkingEffort::High => 24576,
    };
    let budget = preset.min(max_tokens.saturating_sub(1));
    (budget >= MIN_THINKING_BUDGET).then_some(budget)
}

/// The media type Anthropic accepts for an image, if any.
fn image_media_type(mime_type: &str) -> Option<&'static str> {
    match mime_type.to_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/gif" => Some("image/gif"),
        "image/webp" => Some("image/webp"),
        _ => None,
    }
}

fn image_block(data: &str, media_type: &str) -> Value {
    json!({
        "type": "image",
        "source": { "type": "base64", "media_type": media_type, "data": data },
    })
}

fn user_block(part: &ContentPart) -> Value {
    match part {
        ContentPart::Text(text) => json!({ "type": "text", "text": text }),
        ContentPart::Image {
            data: Some(data),
            mime_type,
        } => image_block(
            data,
            if mime_type.is_empty() {
                "image/jpeg"
            } else {
                mime_type
            },
        ),
        ContentPart::Image { data: None, .. } => {
            json!({ "type": "text", "text": "[Image not available]" })
        }
        ContentPart::Audio { .. } => {
            json!({ "type": "text", "text": "[Unsupported content format for Anthropic: audio]" })
        }
        ContentPart::Unsupported(kind) => {
            json!({ "type": "text", "text": format!("[Unsupported content: {kind}]") })
        }
    }
}

/// A plain string when every part is text, otherwise content blocks.
fn user_content(content: &[ContentPart]) -> Value {
    if content
        .iter()
        .all(|part| matches!(part, ContentPart::Text(_)))
    {
        Value::String(text_of(content))
    } else {
        Value::Array(content.iter().map(user_block).collect())
    }
}

/// The `tool_result` block answering `tool_use_id`. Images it can send go
/// with the text; others are replaced by a note.
fn tool_result(message: &ChatMessage, tool_use_id: &str) -> Value {
    let text = text_of(&message.content);
    let had_images = message
        .content
        .iter()
        .any(|part| matches!(part, ContentPart::Image { .. }));
    let images: Vec<Value> = message
        .content
        .iter()
        .filter_map(|part| match part {
            ContentPart::Image {
                data: Some(data),
                mime_type,
            } => image_media_type(mime_type).map(|media_type| image_block(data, media_type)),
            _ => None,
        })
        .collect();

    let content = if !had_images {
        Value::String(text)
    } else if images.is_empty() {
        let note = "[Tool returned image(s) that could not be displayed due to unsupported format or missing data.]";
        Value::String(if text.is_empty() {
            note.to_string()
        } else {
            format!("{text}\n\n{note}")
        })
    } else {
        let mut blocks = Vec::with_capacity(images.len() + 1);
        if !text.is_empty() {
            blocks.push(json!({ "type": "text", "text": text }));
        }
        blocks.extend(images);
        Value::Array(blocks)
    };
    json!({ "type": "tool_result", "tool_use_id": tool_use_id, "content": content })
}

/// A tool call's arguments as the object Anthropic expects; `{}` when they
/// are not a JSON object.
fn tool_input(call: &ToolCall) -> Value {
    match serde_json::from_str::<Value>(&call.function.arguments) {
        Ok(input) if input.is_object() => input,
        _ => {
            log::warn!(
                "Tool call {} has arguments that are not a JSON object; sent as {{}}",
                call.id
            );
            json!({})
        }
    }
}

fn assistant_blocks(message: &ChatMessage) -> Vec<Value> {
    let mut blocks = Vec::new();
    // A thinking block is only valid with the signature Anthropic gave it.
    let thinking = message.thinking.as_deref().filter(|text| !text.is_empty());
    let signature = message
        .thinking_signature
        .as_deref()
        .filter(|signature| !signature.is_empty());
    if let (Some(thinking), Some(signature)) = (thinking, signature) {
        blocks.push(json!({ "type": "thinking", "thinking": thinking, "signature": signature }));
    }
    let text = text_of(&message.content);
    if !text.is_empty() {
        blocks.push(json!({ "type": "text", "text": text }));
    }
    blocks.extend(message.tool_calls.iter().map(|call| {
        json!({
            "type": "tool_use",
            "id": call.id,
            "name": call.function.name,
            "input": tool_input(call),
        })
    }));
    blocks
}

/// Marks the last cacheable block of `message` as a cache breakpoint.
fn apply_cache_breakpoint(message: &mut Value) {
    let content = &mut message["content"];
    if let Some(text) = content.as_str().map(str::to_string) {
        if !text.is_empty() {
            *content = json!([{ "type": "text", "text": text, "cache_control": ephemeral() }]);
        }
        return;
    }
    if let Some(block) = content.as_array_mut().and_then(|blocks| {
        blocks.iter_mut().rev().find(|block| {
            !matches!(
                block["type"].as_str(),
                Some("thinking" | "redacted_thinking")
            )
        })
    }) {
        block["cache_control"] = ephemeral();
    }
}

fn flush_tool_results(out: &mut Vec<Value>, pending: &mut Vec<Value>) {
    if !pending.is_empty() {
        out.push(json!({ "role": "user", "content": std::mem::take(pending) }));
    }
}

pub(crate) fn messages(messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = Vec::with_capacity(messages.len());
    let mut pending_results = Vec::new();

    for message in messages {
        match message.role {
            Role::User => {
                flush_tool_results(&mut out, &mut pending_results);
                out.push(json!({ "role": "user", "content": user_content(&message.content) }));
            }
            Role::Assistant => {
                flush_tool_results(&mut out, &mut pending_results);
                let blocks = assistant_blocks(message);
                if !blocks.is_empty() {
                    out.push(json!({ "role": "assistant", "content": blocks }));
                }
            }
            Role::Tool => {
                let Some(tool_call_id) = message.tool_call_id.as_deref() else {
                    log::warn!("A tool message has no tool_call_id; skipped");
                    continue;
                };
                pending_results.push(tool_result(message, tool_call_id));
            }
        }
    }
    flush_tool_results(&mut out, &mut pending_results);

    if let Some(last) = out.len().checked_sub(1) {
        let middle = last / 2;
        if out.len() >= MIN_MESSAGES_FOR_MIDPOINT_BREAKPOINT && middle > 0 {
            apply_cache_breakpoint(&mut out[middle]);
        }
        apply_cache_breakpoint(&mut out[last]);
    }
    out
}

fn tools(tools: &[crate::types::ToolSpec]) -> Vec<Value> {
    let mut out: Vec<Value> = tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": object_schema(tool),
            })
        })
        .collect();
    // The last tool caches the whole list.
    if let Some(last) = out.last_mut() {
        last["cache_control"] = ephemeral();
    }
    out
}

/// The streaming Messages body for `request`, sent as `model`.
pub(crate) fn request_body(model: &str, request: &ChatRequest) -> Value {
    let max_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": messages(&request.messages),
        "stream": true,
    });
    if let Some(prompt) = request
        .system_prompt
        .as_deref()
        .filter(|prompt| !prompt.is_empty())
    {
        body["system"] = json!([{ "type": "text", "text": prompt, "cache_control": ephemeral() }]);
    }
    match thinking_budget(request.thinking_effort, max_tokens) {
        // Anthropic rejects a custom temperature while thinking.
        Some(budget) => {
            body["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
        }
        None => {
            if let Some(temperature) = request.temperature {
                body["temperature"] = json!(temperature.clamp(0.0, 1.0));
            }
        }
    }
    if !request.tools.is_empty() {
        body["tools"] = Value::Array(tools(&request.tools));
    }
    body
}

#[derive(Default)]
struct PartialToolUse {
    id: String,
    name: String,
    json: String,
    /// The input the start event already carried, used when no delta follows.
    initial_input: Option<String>,
}

/// Accumulates an Anthropic Messages SSE stream.
#[derive(Default)]
pub struct AnthropicStream {
    content: String,
    thinking: String,
    signature: String,
    tool_uses: BTreeMap<u64, PartialToolUse>,
    input_tokens: u64,
    cache_creation_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    output_tokens: u64,
    saw_usage: bool,
    stop_reason: Option<String>,
}

impl AnthropicStream {
    fn read_usage(&mut self, usage: &Value) {
        if !usage.is_object() {
            return;
        }
        self.saw_usage = true;
        let count = |key: &str| usage.get(key).and_then(Value::as_u64);
        if let Some(tokens) = count("input_tokens") {
            self.input_tokens = tokens;
        }
        if let Some(tokens) = count("cache_creation_input_tokens") {
            self.cache_creation_tokens = Some(tokens);
        }
        if let Some(tokens) = count("cache_read_input_tokens") {
            self.cache_read_tokens = Some(tokens);
        }
        if let Some(tokens) = count("output_tokens") {
            self.output_tokens = tokens;
        }
    }
}

fn text_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

impl StreamParser for AnthropicStream {
    fn push_line(&mut self, line: &str) -> Result<bool, String> {
        // `event:` lines repeat the `type` the data carries.
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(false);
        };
        let data = data.trim();
        if data.is_empty() {
            return Ok(false);
        }
        let event: Value = serde_json::from_str(data)
            .map_err(|error| format!("Malformed stream event: {error}"))?;
        let index = event.get("index").and_then(Value::as_u64);

        match event.get("type").and_then(Value::as_str) {
            Some("message_start") => self.read_usage(&event["message"]["usage"]),
            Some("message_delta") => {
                if let Some(reason) = event["delta"].get("stop_reason").and_then(Value::as_str) {
                    self.stop_reason = Some(reason.to_string());
                }
                self.read_usage(&event["usage"]);
            }
            Some("content_block_start") => {
                let block = &event["content_block"];
                if let (Some(index), Some("tool_use")) = (index, block["type"].as_str()) {
                    let initial_input = block
                        .get("input")
                        .filter(|input| input.as_object().is_some_and(|map| !map.is_empty()))
                        .map(Value::to_string);
                    self.tool_uses.insert(
                        index,
                        PartialToolUse {
                            id: text_field(block, "id").to_string(),
                            name: text_field(block, "name").to_string(),
                            json: String::new(),
                            initial_input,
                        },
                    );
                }
            }
            Some("content_block_delta") => {
                let delta = &event["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => self.content.push_str(text_field(delta, "text")),
                    Some("thinking_delta") => self.thinking.push_str(text_field(delta, "thinking")),
                    Some("signature_delta") => {
                        self.signature.push_str(text_field(delta, "signature"))
                    }
                    Some("input_json_delta") => {
                        if let Some(tool_use) = index.and_then(|i| self.tool_uses.get_mut(&i)) {
                            tool_use.json.push_str(text_field(delta, "partial_json"));
                        }
                    }
                    _ => {}
                }
            }
            Some("message_stop") => return Ok(true),
            Some("error") => {
                let message = event["error"]
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| event["error"].to_string());
                return Err(format!(
                    "Provider returned an error in the stream: {message}"
                ));
            }
            _ => {}
        }
        Ok(false)
    }

    fn finish(self: Box<Self>) -> CompletedChat {
        let usage = self.saw_usage.then(|| {
            let prompt_tokens = self.input_tokens
                + self.cache_creation_tokens.unwrap_or(0)
                + self.cache_read_tokens.unwrap_or(0);
            Usage {
                prompt_tokens,
                completion_tokens: self.output_tokens,
                total_tokens: prompt_tokens + self.output_tokens,
                cached_prompt_tokens: self.cache_read_tokens,
                details: None,
            }
        });
        let tool_calls = self
            .tool_uses
            .into_values()
            .filter(|tool_use| !tool_use.name.is_empty())
            .map(|tool_use| ToolCall {
                id: tool_use.id,
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: tool_use.name,
                    arguments: if !tool_use.json.trim().is_empty() {
                        tool_use.json
                    } else {
                        tool_use.initial_input.unwrap_or_else(|| "{}".to_string())
                    },
                },
            })
            .collect();
        // In the OpenAI vocabulary the rest of the client uses.
        let finish_reason = self.stop_reason.map(|reason| {
            match reason.as_str() {
                "end_turn" | "stop_sequence" => "stop",
                "tool_use" => "tool_calls",
                "max_tokens" => "length",
                other => other,
            }
            .to_string()
        });

        CompletedChat {
            content: self.content,
            thinking: self.thinking,
            thinking_signature: (!self.signature.is_empty()).then_some(self.signature),
            tool_calls,
            usage,
            finish_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::LineBuffer;
    use crate::types::ToolSpec;

    fn message(role: Role, content: Vec<ContentPart>) -> ChatMessage {
        ChatMessage {
            role,
            content,
            tool_calls: Vec::new(),
            tool_call_id: None,
            thinking: None,
            thinking_signature: None,
        }
    }

    fn text(value: &str) -> ContentPart {
        ContentPart::Text(value.to_string())
    }

    fn call(id: &str, name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
        }
    }

    fn tool_turn(id: &str, content: Vec<ContentPart>) -> ChatMessage {
        ChatMessage {
            tool_call_id: Some(id.to_string()),
            ..message(Role::Tool, content)
        }
    }

    fn replay(recording: &str) -> CompletedChat {
        let mut lines = LineBuffer::default();
        let mut stream = Box::<AnthropicStream>::default();
        for line in lines.push(recording.as_bytes()) {
            if stream.push_line(&line).expect("line accepted") {
                break;
            }
        }
        stream.finish()
    }

    #[test]
    fn the_body_caches_the_system_prompt_the_tools_and_the_last_message() {
        let request = ChatRequest {
            model: "claude-sonnet-4-5".to_string(),
            system_prompt: Some("You are an agent.".to_string()),
            messages: vec![message(Role::User, vec![text("hi")])],
            tools: vec![
                ToolSpec {
                    name: "a".to_string(),
                    description: "first".to_string(),
                    parameters: json!({ "properties": {} }),
                },
                ToolSpec {
                    name: "b".to_string(),
                    description: "second".to_string(),
                    parameters: json!({ "type": "object" }),
                },
            ],
            temperature: Some(1.6),
            ..ChatRequest::default()
        };

        let body = request_body("claude-sonnet-4-5", &request);

        assert_eq!(body["model"], "claude-sonnet-4-5");
        assert_eq!(body["stream"], true);
        // Anthropic requires an output limit; the GUI's default applies.
        assert_eq!(body["max_tokens"], DEFAULT_MAX_TOKENS);
        assert_eq!(body["system"][0]["text"], "You are an agent.");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        assert!(body["tools"][0].get("cache_control").is_none());
        assert_eq!(body["tools"][1]["cache_control"]["type"], "ephemeral");
        assert_eq!(
            body["messages"][0]["content"],
            json!([{ "type": "text", "text": "hi", "cache_control": { "type": "ephemeral" } }])
        );
        // Anthropic's temperature range is 0 to 1.
        assert_eq!(body["temperature"], 1.0);
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn thinking_effort_sets_a_budget_below_max_tokens_and_drops_temperature() {
        let body = |effort, max_tokens| {
            request_body(
                "m",
                &ChatRequest {
                    thinking_effort: effort,
                    max_tokens,
                    temperature: Some(0.2),
                    ..ChatRequest::default()
                },
            )
        };

        let low = body(ThinkingEffort::Low, None);
        assert_eq!(
            low["thinking"],
            json!({ "type": "enabled", "budget_tokens": 1024 })
        );
        assert!(low.get("temperature").is_none());
        assert_eq!(
            body(ThinkingEffort::Auto, Some(32_000))["thinking"]["budget_tokens"],
            8192
        );
        // High asks for more than the default limit, so it is cut below it.
        assert_eq!(
            body(ThinkingEffort::High, None)["thinking"]["budget_tokens"],
            DEFAULT_MAX_TOKENS - 1
        );
        // No room for the smallest budget: no thinking, temperature kept.
        let tight = body(ThinkingEffort::High, Some(512));
        assert!(tight.get("thinking").is_none());
        assert_eq!(tight["temperature"], 0.2);
        assert!(body(ThinkingEffort::Off, None).get("thinking").is_none());
    }

    #[test]
    fn tool_calls_and_their_results_keep_their_pairing_in_one_user_turn() {
        let mut assistant = message(Role::Assistant, vec![text("Reading both.")]);
        assistant.thinking = Some("need two files".to_string());
        assistant.thinking_signature = Some("sig-1".to_string());
        assistant.tool_calls = vec![
            call("toolu_1", "workspace__readFile", r#"{"path":"a.md"}"#),
            call("toolu_2", "workspace__readFile", "not json"),
        ];

        let out = messages(&[
            message(Role::User, vec![text("read a and b")]),
            assistant,
            tool_turn("toolu_1", vec![text("A")]),
            tool_turn("toolu_2", vec![text("B")]),
            message(Role::User, vec![text("thanks")]),
        ]);

        assert_eq!(out.len(), 4);
        let blocks = out[1]["content"].as_array().unwrap();
        assert_eq!(
            blocks[0],
            json!({ "type": "thinking", "thinking": "need two files", "signature": "sig-1" })
        );
        assert_eq!(
            blocks[1],
            json!({ "type": "text", "text": "Reading both." })
        );
        assert_eq!(blocks[2]["type"], "tool_use");
        assert_eq!(blocks[2]["input"], json!({ "path": "a.md" }));
        // Arguments that are not an object still give a valid input.
        assert_eq!(blocks[3]["input"], json!({}));

        assert_eq!(out[2]["role"], "user");
        let results = out[2]["content"].as_array().unwrap();
        assert_eq!(
            results[0],
            json!({ "type": "tool_result", "tool_use_id": "toolu_1", "content": "A" })
        );
        assert_eq!(results[1]["tool_use_id"], "toolu_2");
    }

    #[test]
    fn thinking_without_a_signature_is_not_sent() {
        let mut assistant = message(Role::Assistant, vec![text("answer")]);
        assistant.thinking = Some("from another provider".to_string());

        let out = messages(&[message(Role::User, vec![text("q")]), assistant]);

        let blocks = out[1]["content"].as_array().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
    }

    #[test]
    fn tool_images_ride_in_the_result_and_bad_ones_become_a_note() {
        let image = |mime: &str| ContentPart::Image {
            data: Some("QUJD".to_string()),
            mime_type: mime.to_string(),
        };
        let out = messages(&[
            tool_turn("toolu_1", vec![text("shot"), image("image/jpg")]),
            tool_turn("toolu_2", vec![image("image/tiff")]),
        ]);

        let results = out[0]["content"].as_array().unwrap();
        let blocks = results[0]["content"].as_array().unwrap();
        assert_eq!(blocks[0], json!({ "type": "text", "text": "shot" }));
        assert_eq!(blocks[1]["source"]["media_type"], "image/jpeg");
        let note = results[1]["content"].as_str().unwrap();
        assert!(note.contains("could not be displayed"), "{note}");
    }

    #[test]
    fn user_media_uses_content_blocks() {
        let out = messages(&[message(
            Role::User,
            vec![
                text("look"),
                ContentPart::Image {
                    data: Some("QUJD".to_string()),
                    mime_type: "image/png".to_string(),
                },
                ContentPart::Audio {
                    data: Some("UklG".to_string()),
                    mime_type: "audio/wav".to_string(),
                },
            ],
        )]);
        let blocks = out[0]["content"].as_array().unwrap();
        assert_eq!(blocks[1]["source"]["data"], "QUJD");
        assert!(blocks[2]["text"].as_str().unwrap().contains("audio"));
    }

    #[test]
    fn a_long_history_also_caches_its_middle() {
        let history: Vec<ChatMessage> = (0..10)
            .map(|i| {
                let role = if i % 2 == 0 {
                    Role::User
                } else {
                    Role::Assistant
                };
                message(role, vec![text(&format!("turn {i}"))])
            })
            .collect();

        let out = messages(&history);

        let cached: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(_, m)| m["content"][0].get("cache_control").is_some())
            .map(|(i, _)| i)
            .collect();
        assert_eq!(cached, vec![4, 9]);
        assert!(messages(&history[..3])[1]["content"].as_array().unwrap()[0]
            .get("cache_control")
            .is_none());
    }

    #[test]
    fn recorded_thinking_stream_keeps_reasoning_signature_text_and_cache_usage() {
        let chat = replay(include_str!(
            "../tests/fixtures/anthropic_thinking_text.sse"
        ));

        assert_eq!(chat.thinking, "The user wants 2+2. That is 4.");
        assert_eq!(
            chat.thinking_signature.as_deref(),
            Some("EqQBCgIYAhIM1gbcDa9GJwZA2b3hGgxBdjrkzLoky3dl1pkiMOYds")
        );
        assert_eq!(chat.content, "2 + 2 = 4.");
        assert_eq!(chat.finish_reason.as_deref(), Some("stop"));
        let usage = chat.usage.expect("usage");
        // Cached input counts toward the prompt, as in the GUI.
        assert_eq!(usage.prompt_tokens, 2_060);
        assert_eq!(usage.cached_prompt_tokens, Some(2_048));
        assert_eq!(usage.completion_tokens, 42);
        assert_eq!(usage.total_tokens, 2_102);
    }

    #[test]
    fn recorded_tool_use_stream_joins_input_deltas_per_block() {
        let chat = replay(include_str!("../tests/fixtures/anthropic_tool_use.sse"));

        assert_eq!(chat.content, "Let me check.");
        assert_eq!(chat.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(chat.tool_calls.len(), 2);
        assert_eq!(chat.tool_calls[0].id, "toolu_01A09q90qw90lq917835lq9");
        assert_eq!(chat.tool_calls[0].function.name, "workspace__readFile");
        assert_eq!(
            chat.tool_calls[0].function.arguments,
            r#"{"path": "notes/todo.md"}"#
        );
        // A call without arguments still gets a JSON object.
        assert_eq!(chat.tool_calls[1].function.arguments, "{}");
        assert_eq!(chat.usage.unwrap().prompt_tokens, 1_500);
        assert!(chat.thinking_signature.is_none());
    }

    #[test]
    fn an_input_given_only_at_block_start_is_kept() {
        let mut stream = AnthropicStream::default();
        stream
            .push_line(r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t","name":"n","input":{"k":1}}}"#)
            .unwrap();
        let chat = Box::new(stream).finish();
        assert_eq!(chat.tool_calls[0].function.arguments, r#"{"k":1}"#);
    }

    #[test]
    fn an_error_event_or_bad_json_fails_the_stream() {
        let mut stream = AnthropicStream::default();
        let error = stream
            .push_line(r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#)
            .unwrap_err();
        assert!(error.contains("Overloaded"), "{error}");
        assert!(AnthropicStream::default().push_line("data: {nope").is_err());
    }
}
