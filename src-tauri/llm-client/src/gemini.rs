//! Gemini: the Generative Language API (`streamGenerateContent`).
//!
//! Mirrors the frontend's `GeminiService` and its mapper
//! (`src/lib/ai-service/gemini/`): system instruction, function declarations
//! with Gemini's schema subset, consecutive tool results batched into one user
//! turn, thought signatures on the first function call, and the GUI's safety
//! settings.

use crate::openai::{is_media, text_of, tool_name_for};
use crate::provider::{ChatProvider, Endpoint};
use crate::stream::StreamParser;
use crate::types::{
    ChatMessage, ChatRequest, CompletedChat, ContentPart, Role, ThinkingEffort, ToolCall,
    ToolCallFunction, ToolSpec, Usage, UsageDetails,
};
use serde_json::{json, Map, Value};

/// Sent with a function call that has no signature, so Gemini 3 accepts
/// history it did not produce (same value as the GUI).
const DUMMY_THOUGHT_SIGNATURE: &str = "skip_thought_signature_validator";

#[derive(Debug, Clone)]
pub struct Gemini {
    pub endpoint: Endpoint,
}

impl ChatProvider for Gemini {
    fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn request_url(&self, model: &str) -> String {
        format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.endpoint.base_url.trim_end_matches('/'),
            model.trim_start_matches("models/")
        )
    }

    fn auth_header(&self, api_key: &str) -> (&'static str, String) {
        ("x-goog-api-key", api_key.to_string())
    }

    fn build_body(&self, request: &ChatRequest) -> Value {
        request_body(request)
    }

    fn stream_parser(&self) -> Box<dyn StreamParser> {
        Box::<GeminiStream>::default()
    }
}

/// The GUI's thinking budgets; `-1` lets the model decide.
fn thinking_budget(effort: ThinkingEffort) -> Option<i64> {
    match effort {
        ThinkingEffort::Off => None,
        ThinkingEffort::Low => Some(1024),
        ThinkingEffort::Medium => Some(8192),
        ThinkingEffort::High => Some(24576),
        ThinkingEffort::Auto => Some(-1),
    }
}

fn part(content: &ContentPart) -> Value {
    match content {
        ContentPart::Text(text) => json!({ "text": text }),
        ContentPart::Image {
            data: Some(data),
            mime_type,
        } => inline_data(data, mime_type, "image/jpeg"),
        ContentPart::Audio {
            data: Some(data),
            mime_type,
        } => inline_data(data, mime_type, "audio/mp3"),
        ContentPart::Image { data: None, .. } => json!({ "text": "[Image not available]" }),
        ContentPart::Audio { data: None, .. } => json!({ "text": "[Audio not available]" }),
        ContentPart::Unsupported(kind) => {
            json!({ "text": format!("[Unsupported content: {kind}]") })
        }
    }
}

fn inline_data(data: &str, mime_type: &str, fallback: &str) -> Value {
    let mime_type = if mime_type.is_empty() {
        fallback
    } else {
        mime_type
    };
    json!({ "inlineData": { "mimeType": mime_type, "data": data } })
}

/// One text part when every part is text, otherwise one part each.
fn user_parts(content: &[ContentPart]) -> Vec<Value> {
    if content
        .iter()
        .all(|part| matches!(part, ContentPart::Text(_)))
    {
        vec![json!({ "text": text_of(content) })]
    } else {
        content.iter().map(part).collect()
    }
}

fn call_args(call: &ToolCall) -> Value {
    serde_json::from_str::<Value>(&call.function.arguments)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn model_parts(message: &ChatMessage) -> Vec<Value> {
    let signature = message
        .thinking_signature
        .as_deref()
        .filter(|signature| !signature.is_empty());
    let mut parts = Vec::new();
    let text = text_of(&message.content);
    if !text.is_empty() {
        let mut text_part = json!({ "text": text });
        if message.tool_calls.is_empty() {
            if let Some(signature) = signature {
                text_part["thoughtSignature"] = json!(signature);
            }
        }
        parts.push(text_part);
    }
    for (index, call) in message.tool_calls.iter().enumerate() {
        let mut call_part = json!({
            "functionCall": { "name": call.function.name, "args": call_args(call) },
        });
        // Gemini wants the signature on the first call of a turn only.
        if index == 0 {
            call_part["thoughtSignature"] = json!(signature.unwrap_or(DUMMY_THOUGHT_SIGNATURE));
        }
        parts.push(call_part);
    }
    parts
}

/// The parts answering one tool message, which follows `earlier`.
fn tool_response_parts(earlier: &[ChatMessage], message: &ChatMessage) -> Vec<Value> {
    let Some(tool_call_id) = message.tool_call_id.as_deref() else {
        log::warn!("A tool message has no tool_call_id; skipped");
        return Vec::new();
    };
    // Gemini matches a response to its call by function name.
    let Some(name) = tool_name_for(earlier, tool_call_id) else {
        return vec![json!({ "text": text_of(&message.content) })];
    };
    let mut parts = vec![json!({
        "functionResponse": {
            "id": tool_call_id,
            "name": name,
            "response": { "result": text_of(&message.content) },
        },
    })];
    let media: Vec<&ContentPart> = message.content.iter().filter(|p| is_media(p)).collect();
    if !media.is_empty() {
        parts.push(json!({
            "text": format!("[Image/Audio output from tool \"{name}\" (ID: {tool_call_id})]"),
        }));
        parts.extend(media.into_iter().map(part));
    }
    parts
}

pub(crate) fn contents(messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = Vec::with_capacity(messages.len() + 1);
    let mut index = 0;
    while index < messages.len() {
        let message = &messages[index];
        match message.role {
            Role::Tool => {
                // Every response to a turn's calls goes in one user turn.
                let mut parts = Vec::new();
                while index < messages.len() && messages[index].role == Role::Tool {
                    parts.extend(tool_response_parts(&messages[..index], &messages[index]));
                    index += 1;
                }
                if !parts.is_empty() {
                    out.push(json!({ "role": "user", "parts": parts }));
                }
                continue;
            }
            Role::User => {
                out.push(json!({ "role": "user", "parts": user_parts(&message.content) }));
            }
            Role::Assistant => {
                let parts = model_parts(message);
                if !parts.is_empty() {
                    out.push(json!({ "role": "model", "parts": parts }));
                }
            }
        }
        index += 1;
    }
    // Gemini rejects a conversation that does not start with the user.
    if out
        .first()
        .is_some_and(|first| first["role"].as_str() != Some("user"))
    {
        out.insert(0, json!({ "role": "user", "parts": [{ "text": "." }] }));
    }
    out
}

/// A JSON Schema in the subset Gemini's function declarations accept.
pub(crate) fn gemini_schema(schema: &Value) -> Value {
    let with_description = |mut converted: Value| {
        if let Some(description) = schema.get("description").and_then(Value::as_str) {
            converted["description"] = json!(description);
        }
        converted
    };
    // Gemini reads oneOf as anyOf.
    if let Some(variants) = schema
        .get("oneOf")
        .or_else(|| schema.get("anyOf"))
        .and_then(Value::as_array)
        .filter(|variants| !variants.is_empty())
    {
        let any_of: Vec<Value> = variants.iter().map(gemini_schema).collect();
        return with_description(json!({ "anyOf": any_of }));
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => {
            let mut converted = json!({ "type": "STRING" });
            if let Some(values) = schema.get("enum").filter(|values| values.is_array()) {
                converted["enum"] = values.clone();
            } else if let Some(value) = schema.get("const").and_then(Value::as_str) {
                converted["enum"] = json!([value]);
            }
            with_description(converted)
        }
        Some("number" | "integer") => with_description(json!({ "type": "NUMBER" })),
        Some("boolean") => with_description(json!({ "type": "BOOLEAN" })),
        Some("null") => with_description(json!({ "type": "STRING" })),
        Some("array") if schema.get("items").is_some() => {
            let items = match &schema["items"] {
                Value::Array(items) => items.first().map(gemini_schema),
                items => Some(gemini_schema(items)),
            };
            with_description(json!({
                "type": "ARRAY",
                "items": items.unwrap_or_else(|| json!({ "type": "STRING" })),
            }))
        }
        Some("object") if schema.get("properties").is_some_and(Value::is_object) => {
            let properties = converted_properties(schema);
            let required: Vec<&Value> = schema
                .get("required")
                .and_then(Value::as_array)
                .map(|keys| {
                    keys.iter()
                        .filter(|key| key.as_str().is_some_and(|k| properties.contains_key(k)))
                        .collect()
                })
                .unwrap_or_default();
            let mut converted = json!({ "type": "OBJECT", "properties": properties });
            if !required.is_empty() {
                converted["required"] = json!(required);
            }
            with_description(converted)
        }
        _ => json!({ "type": "STRING" }),
    }
}

fn converted_properties(schema: &Value) -> Map<String, Value> {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .map(|(key, property)| (key.clone(), gemini_schema(property)))
                .collect()
        })
        .unwrap_or_default()
}

fn function_declaration(tool: &ToolSpec) -> Value {
    let schema = &tool.parameters;
    let mut parameters = json!({
        "type": "OBJECT",
        "properties": converted_properties(schema),
        "required": schema
            .get("required")
            .filter(|required| required.is_array())
            .cloned()
            .unwrap_or_else(|| json!([])),
    });
    if let Some(description) = schema.get("description").and_then(Value::as_str) {
        parameters["description"] = json!(description);
    }
    json!({ "name": tool.name, "description": tool.description, "parameters": parameters })
}

/// Blocking off for the main harm categories, as in the GUI, so agent work
/// is not stopped by false positives.
fn safety_settings() -> Value {
    let categories = [
        "HARM_CATEGORY_HARASSMENT",
        "HARM_CATEGORY_HATE_SPEECH",
        "HARM_CATEGORY_SEXUALLY_EXPLICIT",
        "HARM_CATEGORY_DANGEROUS_CONTENT",
    ];
    Value::Array(
        categories
            .iter()
            .map(|category| json!({ "category": category, "threshold": "BLOCK_NONE" }))
            .collect(),
    )
}

/// The `streamGenerateContent` body for `request`. The model is in the URL.
pub(crate) fn request_body(request: &ChatRequest) -> Value {
    let mut generation = json!({ "responseMimeType": "text/plain" });
    if let Some(max_tokens) = request.max_tokens {
        generation["maxOutputTokens"] = json!(max_tokens);
    }
    if let Some(temperature) = request.temperature {
        generation["temperature"] = json!(temperature);
    }
    if let Some(budget) = thinking_budget(request.thinking_effort) {
        generation["thinkingConfig"] = json!({ "thinkingBudget": budget, "includeThoughts": true });
    }

    let mut body = json!({
        "contents": contents(&request.messages),
        "generationConfig": generation,
        "safetySettings": safety_settings(),
    });
    if let Some(prompt) = request
        .system_prompt
        .as_deref()
        .filter(|prompt| !prompt.is_empty())
    {
        body["systemInstruction"] = json!({ "parts": [{ "text": prompt }] });
    }
    if !request.tools.is_empty() {
        let declarations: Vec<Value> = request.tools.iter().map(function_declaration).collect();
        body["tools"] = json!([{ "functionDeclarations": declarations }]);
    }
    body
}

/// Accumulates a Gemini `streamGenerateContent?alt=sse` stream. The stream
/// has no end marker; it ends with the response.
#[derive(Default)]
pub struct GeminiStream {
    content: String,
    thinking: String,
    /// The signature on the turn's first function call.
    call_signature: Option<String>,
    /// The last signature on any part, for a turn without calls.
    last_signature: Option<String>,
    tool_calls: Vec<ToolCall>,
    usage: Option<Usage>,
    finish_reason: Option<String>,
}

fn usage_of(metadata: &Value) -> Usage {
    let count = |key: &str| metadata.get(key).and_then(Value::as_u64);
    let prompt_tokens = count("promptTokenCount").unwrap_or(0);
    let completion_tokens = count("candidatesTokenCount").unwrap_or(0);
    Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
        cached_prompt_tokens: count("cachedContentTokenCount"),
        details: count("thoughtsTokenCount").map(|tokens| UsageDetails {
            reasoning_tokens: Some(tokens),
        }),
    }
}

impl GeminiStream {
    fn read_part(&mut self, part: &Value) {
        let signature = part.get("thoughtSignature").and_then(Value::as_str);
        if let Some(signature) = signature {
            self.last_signature = Some(signature.to_string());
        }
        if let Some(call) = part.get("functionCall") {
            let Some(name) = call
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
            else {
                return;
            };
            if self.tool_calls.is_empty() {
                self.call_signature = signature.map(str::to_string);
            }
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4().simple()));
            let arguments = call
                .get("args")
                .filter(|args| !args.is_null())
                .map(Value::to_string)
                .unwrap_or_else(|| "{}".to_string());
            self.tool_calls.push(ToolCall {
                id,
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: name.to_string(),
                    arguments,
                },
            });
        } else if let Some(text) = part.get("text").and_then(Value::as_str) {
            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                self.thinking.push_str(text);
            } else {
                self.content.push_str(text);
            }
        }
    }
}

impl StreamParser for GeminiStream {
    fn push_line(&mut self, line: &str) -> Result<bool, String> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(false);
        };
        let data = data.trim();
        if data.is_empty() {
            return Ok(false);
        }
        let chunk: Value = serde_json::from_str(data)
            .map_err(|error| format!("Malformed stream chunk: {error}"))?;
        if let Some(error) = chunk.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| error.to_string());
            return Err(format!(
                "Provider returned an error in the stream: {message}"
            ));
        }
        if let Some(metadata) = chunk.get("usageMetadata").filter(|m| m.is_object()) {
            self.usage = Some(usage_of(metadata));
        }
        let candidate = &chunk["candidates"][0];
        if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.to_string());
        }
        if let Some(parts) = candidate["content"]["parts"].as_array() {
            for part in parts {
                self.read_part(part);
            }
        }
        Ok(false)
    }

    fn finish(self: Box<Self>) -> CompletedChat {
        // In the OpenAI vocabulary the rest of the client uses.
        let has_calls = !self.tool_calls.is_empty();
        let finish_reason = self.finish_reason.map(|reason| match reason.as_str() {
            "STOP" if has_calls => "tool_calls".to_string(),
            "STOP" => "stop".to_string(),
            "MAX_TOKENS" => "length".to_string(),
            other => other.to_lowercase(),
        });
        CompletedChat {
            content: self.content,
            thinking: self.thinking,
            thinking_signature: self.call_signature.or(self.last_signature),
            tool_calls: self.tool_calls,
            usage: self.usage,
            finish_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::LineBuffer;

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
        let mut stream = Box::<GeminiStream>::default();
        for line in lines.push(recording.as_bytes()) {
            stream.push_line(&line).expect("line accepted");
        }
        stream.finish()
    }

    #[test]
    fn the_body_carries_system_instruction_generation_config_and_safety_settings() {
        let request = ChatRequest {
            model: "gemini-2.5-flash".to_string(),
            system_prompt: Some("You are an agent.".to_string()),
            messages: vec![message(Role::User, vec![text("hi")])],
            tools: vec![ToolSpec {
                name: "workspace__readFile".to_string(),
                description: "Read a file".to_string(),
                parameters: json!({
                    "type": "object",
                    "description": "Arguments",
                    "properties": { "path": { "type": "string", "description": "File path" } },
                    "required": ["path"],
                }),
            }],
            max_tokens: Some(4096),
            temperature: Some(0.3),
            thinking_effort: ThinkingEffort::Auto,
        };

        let body = request_body(&request);

        assert_eq!(
            body["systemInstruction"],
            json!({ "parts": [{ "text": "You are an agent." }] })
        );
        assert_eq!(
            body["contents"],
            json!([{ "role": "user", "parts": [{ "text": "hi" }] }])
        );
        let generation = &body["generationConfig"];
        assert_eq!(generation["maxOutputTokens"], 4096);
        assert_eq!(generation["temperature"], 0.3);
        assert_eq!(
            generation["thinkingConfig"],
            json!({ "thinkingBudget": -1, "includeThoughts": true })
        );
        assert_eq!(body["safetySettings"].as_array().unwrap().len(), 4);
        assert_eq!(body["safetySettings"][0]["threshold"], "BLOCK_NONE");
        let declaration = &body["tools"][0]["functionDeclarations"][0];
        assert_eq!(declaration["name"], "workspace__readFile");
        assert_eq!(declaration["parameters"]["type"], "OBJECT");
        assert_eq!(declaration["parameters"]["description"], "Arguments");
        assert_eq!(
            declaration["parameters"]["properties"]["path"],
            json!({ "type": "STRING", "description": "File path" })
        );
        assert_eq!(declaration["parameters"]["required"], json!(["path"]));
    }

    #[test]
    fn effort_maps_to_the_gui_budgets_and_off_sends_none() {
        let budget = |effort| {
            request_body(&ChatRequest {
                thinking_effort: effort,
                ..ChatRequest::default()
            })["generationConfig"]
                .get("thinkingConfig")
                .map(|config| config["thinkingBudget"].clone())
        };
        assert_eq!(budget(ThinkingEffort::Off), None);
        assert_eq!(budget(ThinkingEffort::Low), Some(json!(1024)));
        assert_eq!(budget(ThinkingEffort::Medium), Some(json!(8192)));
        assert_eq!(budget(ThinkingEffort::High), Some(json!(24576)));
    }

    #[test]
    fn schemas_are_cut_down_to_what_gemini_accepts() {
        let schema = json!({
            "type": "object",
            "properties": {
                "mode": { "type": "string", "enum": ["a", "b"] },
                "fixed": { "type": "string", "const": "x" },
                "count": { "type": "integer" },
                "tags": { "type": "array", "items": { "type": "boolean" } },
                "either": { "oneOf": [{ "type": "string" }, { "type": "number" }] },
                "nothing": { "type": "null" },
                "free": {},
            },
            "required": ["mode", "missing"],
        });

        let converted = gemini_schema(&schema);

        let properties = &converted["properties"];
        assert_eq!(properties["mode"]["enum"], json!(["a", "b"]));
        assert_eq!(properties["fixed"]["enum"], json!(["x"]));
        assert_eq!(properties["count"]["type"], "NUMBER");
        assert_eq!(properties["tags"]["items"]["type"], "BOOLEAN");
        assert_eq!(
            properties["either"]["anyOf"],
            json!([{ "type": "STRING" }, { "type": "NUMBER" }])
        );
        assert_eq!(properties["nothing"]["type"], "STRING");
        assert_eq!(properties["free"]["type"], "STRING");
        // Only keys that exist stay required.
        assert_eq!(converted["required"], json!(["mode"]));
    }

    #[test]
    fn calls_carry_a_signature_and_their_results_are_batched_into_one_user_turn() {
        let mut assistant = message(Role::Assistant, Vec::new());
        assistant.thinking_signature = Some("sig-1".to_string());
        assistant.tool_calls = vec![
            call("call_1", "workspace__readFile", r#"{"path":"a.md"}"#),
            call("call_2", "media__captureScreen", "{}"),
        ];

        let out = contents(&[
            message(Role::User, vec![text("go")]),
            assistant,
            tool_turn("call_1", vec![text("A")]),
            tool_turn(
                "call_2",
                vec![ContentPart::Image {
                    data: Some("QUJD".to_string()),
                    mime_type: "image/png".to_string(),
                }],
            ),
            tool_turn("call_unknown", vec![text("orphan")]),
        ]);

        assert_eq!(out.len(), 3);
        let calls = out[1]["parts"].as_array().unwrap();
        assert_eq!(out[1]["role"], "model");
        assert_eq!(calls[0]["functionCall"]["args"], json!({ "path": "a.md" }));
        assert_eq!(calls[0]["thoughtSignature"], "sig-1");
        assert!(calls[1].get("thoughtSignature").is_none());

        let responses = out[2]["parts"].as_array().unwrap();
        assert_eq!(out[2]["role"], "user");
        assert_eq!(
            responses[0],
            json!({ "functionResponse": {
                "id": "call_1", "name": "workspace__readFile", "response": { "result": "A" },
            } })
        );
        assert_eq!(
            responses[1]["functionResponse"]["name"],
            "media__captureScreen"
        );
        assert!(responses[2]["text"]
            .as_str()
            .unwrap()
            .contains("media__captureScreen"));
        assert_eq!(responses[3]["inlineData"]["mimeType"], "image/png");
        // A result whose call is unknown is kept as text.
        assert_eq!(responses[4], json!({ "text": "orphan" }));
    }

    #[test]
    fn calls_without_a_signature_get_the_dummy_one_and_text_keeps_its_own() {
        let mut calling = message(Role::Assistant, Vec::new());
        calling.tool_calls = vec![call("c", "t", "{}")];
        let mut answering = message(Role::Assistant, vec![text("done")]);
        answering.thinking_signature = Some("sig-2".to_string());

        let out = contents(&[calling, answering]);

        // Gemini needs the user to speak first.
        assert_eq!(
            out[0],
            json!({ "role": "user", "parts": [{ "text": "." }] })
        );
        assert_eq!(
            out[1]["parts"][0]["thoughtSignature"],
            DUMMY_THOUGHT_SIGNATURE
        );
        assert_eq!(
            out[2]["parts"][0],
            json!({ "text": "done", "thoughtSignature": "sig-2" })
        );
    }

    #[test]
    fn the_url_names_the_model_and_streams_sse() {
        let provider = Gemini {
            endpoint: Endpoint {
                base_url: "https://generativelanguage.googleapis.com/v1beta/".to_string(),
                api_key: None,
            },
        };
        assert_eq!(
            provider.request_url("models/gemini-2.5-pro"),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn recorded_thinking_stream_splits_thoughts_from_the_answer() {
        let chat = replay(include_str!("../tests/fixtures/gemini_thinking_text.sse"));

        assert_eq!(chat.thinking, "The user wants 2+2. That is 4.");
        assert_eq!(chat.content, "2 + 2 = 4.");
        assert_eq!(chat.finish_reason.as_deref(), Some("stop"));
        assert_eq!(
            chat.thinking_signature.as_deref(),
            Some("CiQBVKhc7xQ2mO0rB9mCxWJ5c2dGDUaTI3T0dHjqf0F7sJjXm1c")
        );
        let usage = chat.usage.expect("usage");
        assert_eq!(usage.prompt_tokens, 31);
        assert_eq!(usage.completion_tokens, 7);
        assert_eq!(usage.total_tokens, 38);
        assert_eq!(usage.cached_prompt_tokens, Some(16));
        assert_eq!(usage.details.and_then(|d| d.reasoning_tokens), Some(28));
        assert!(chat.tool_calls.is_empty());
    }

    #[test]
    fn recorded_parallel_function_calls_get_ids_and_the_first_signature() {
        let chat = replay(include_str!("../tests/fixtures/gemini_function_calls.sse"));

        assert_eq!(chat.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(chat.tool_calls.len(), 2);
        assert_eq!(chat.tool_calls[0].function.name, "workspace__readFile");
        assert_eq!(
            chat.tool_calls[0].function.arguments,
            r#"{"path":"notes/todo.md"}"#
        );
        assert_eq!(chat.tool_calls[1].function.name, "workspace__listDirectory");
        // Gemini sends no call ids; the agent loop needs distinct ones.
        assert!(chat.tool_calls[0].id.starts_with("call_"));
        assert_ne!(chat.tool_calls[0].id, chat.tool_calls[1].id);
        assert_eq!(
            chat.thinking_signature.as_deref(),
            Some("CiwBVKhc7wF0nYxS3mQvL4Ld2fGd8kXo1ZpQe2Rk9M0cJtq1sX7uH2Ab")
        );
        assert_eq!(chat.usage.unwrap().prompt_tokens, 2_048);
    }

    #[test]
    fn an_error_chunk_or_bad_json_fails_the_stream() {
        let mut stream = GeminiStream::default();
        let error = stream
            .push_line(r#"data: {"error":{"code":429,"message":"Resource has been exhausted","status":"RESOURCE_EXHAUSTED"}}"#)
            .unwrap_err();
        assert!(error.contains("exhausted"), "{error}");
        assert!(GeminiStream::default().push_line("data: {nope").is_err());
    }
}
