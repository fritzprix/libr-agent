//! Reads a streamed chat answer into one assistant turn.

use crate::types::{
    CompletedChat, StreamDelta, ToolCall, ToolCallDelta, ToolCallFunction, Usage, UsageDetails,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// What one stream line produced.
#[derive(Debug, Default, PartialEq)]
pub struct StreamStep {
    /// The stream says it is done.
    pub done: bool,
    /// What the line added, in order. Empty for a line that carries nothing.
    pub deltas: Vec<StreamDelta>,
}

impl StreamStep {
    /// A line that added nothing; `done` is whether the stream already ended.
    pub fn idle(done: bool) -> Self {
        Self {
            done,
            deltas: Vec::new(),
        }
    }

    /// A line that added `delta` (dropped when it is empty).
    pub fn with(done: bool, delta: StreamDelta) -> Self {
        Self {
            done,
            deltas: if delta.is_empty() {
                Vec::new()
            } else {
                vec![delta]
            },
        }
    }
}

/// Turns the lines of one response stream into a [`CompletedChat`].
///
/// [`OpenAiStream`] is the default; a provider with a different stream format
/// supplies its own through [`crate::ChatProvider::stream_parser`].
///
/// The parser reports each increment as a [`StreamDelta`] while it keeps its
/// own running totals; [`StreamParser::finish`] stays the single source of the
/// final answer, so the deltas are a live view and never need to be re-added.
pub trait StreamParser: Send {
    /// Feed one line. The step says whether the stream is done and what the
    /// line added.
    fn push_line(&mut self, line: &str) -> Result<StreamStep, String>;
    /// Everything the stream produced.
    fn finish(self: Box<Self>) -> CompletedChat;
}

/// Splits a byte stream into complete lines; a line can span network chunks.
#[derive(Default)]
pub(crate) struct LineBuffer {
    pending: Vec<u8>,
}

impl LineBuffer {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let text = String::from_utf8_lossy(&line);
            lines.push(text.trim_end_matches(['\n', '\r']).to_string());
        }
        lines
    }

    /// Whatever is left when the stream ends without a final newline.
    pub(crate) fn finish(self) -> Option<String> {
        (!self.pending.is_empty()).then(|| String::from_utf8_lossy(&self.pending).to_string())
    }
}

#[derive(Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// Accumulates an OpenAI chat-completions SSE stream.
#[derive(Default)]
pub struct OpenAiStream {
    content: String,
    thinking: String,
    tool_calls: BTreeMap<u64, PartialToolCall>,
    usage: Option<Usage>,
    finish_reason: Option<String>,
    done: bool,
}

fn token_count(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn usage_of(usage: &Value) -> Usage {
    let cached_prompt_tokens = usage
        .get("prompt_tokens_details")
        .and_then(|details| token_count(details, "cached_tokens"))
        .or_else(|| token_count(usage, "prompt_cache_hit_tokens"));
    let reasoning_tokens = usage
        .get("completion_tokens_details")
        .and_then(|details| token_count(details, "reasoning_tokens"));
    Usage {
        prompt_tokens: token_count(usage, "prompt_tokens").unwrap_or(0),
        completion_tokens: token_count(usage, "completion_tokens").unwrap_or(0),
        total_tokens: token_count(usage, "total_tokens").unwrap_or(0),
        cached_prompt_tokens,
        details: reasoning_tokens.map(|tokens| UsageDetails {
            reasoning_tokens: Some(tokens),
        }),
    }
}

impl StreamParser for OpenAiStream {
    fn push_line(&mut self, line: &str) -> Result<StreamStep, String> {
        let Some(data) = line.strip_prefix("data:") else {
            // Comments, `event:` lines, and blank separators carry nothing here.
            return Ok(StreamStep::idle(self.done));
        };
        let data = data.trim();
        if data.is_empty() {
            return Ok(StreamStep::idle(self.done));
        }
        if data == "[DONE]" {
            self.done = true;
            return Ok(StreamStep::idle(true));
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

        let mut added = StreamDelta::default();
        if let Some(usage) = chunk.get("usage").filter(|usage| usage.is_object()) {
            let usage = usage_of(usage);
            self.usage = Some(usage.clone());
            added.usage = Some(usage);
        }

        let Some(choice) = chunk.get("choices").and_then(|choices| choices.get(0)) else {
            return Ok(StreamStep::with(false, added));
        };
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.to_string());
            added.finish_reason = Some(reason.to_string());
        }
        let Some(delta) = choice.get("delta") else {
            return Ok(StreamStep::with(false, added));
        };

        // vLLM-served Qwen models use `reasoning`; others use `reasoning_content`.
        for key in ["reasoning_content", "reasoning"] {
            if let Some(text) = delta.get(key).and_then(Value::as_str) {
                if !text.is_empty() {
                    self.thinking.push_str(text);
                    added.thinking = Some(text.to_string());
                    break;
                }
            }
        }
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            self.content.push_str(text);
            if !text.is_empty() {
                added.content = Some(text.to_string());
            }
        }
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let Some(index) = call.get("index").and_then(Value::as_u64) else {
                    continue;
                };
                let partial = self.tool_calls.entry(index).or_default();
                let mut fragment = ToolCallDelta {
                    index,
                    ..ToolCallDelta::default()
                };
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    if !id.is_empty() {
                        partial.id = id.to_string();
                        fragment.id = Some(id.to_string());
                    }
                }
                if let Some(function) = call.get("function") {
                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                        partial.name.push_str(name);
                        if !name.is_empty() {
                            fragment.function.name = Some(name.to_string());
                        }
                    }
                    if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                        partial.arguments.push_str(arguments);
                        fragment.function.arguments = arguments.to_string();
                    }
                }
                if fragment.id.is_some()
                    || fragment.function.name.is_some()
                    || !fragment.function.arguments.is_empty()
                {
                    added.tool_calls.push(fragment);
                }
            }
        }

        Ok(StreamStep::with(false, added))
    }

    fn finish(self: Box<Self>) -> CompletedChat {
        let tool_calls = self
            .tool_calls
            .into_iter()
            .filter(|(_, call)| !call.name.is_empty())
            .map(|(index, call)| ToolCall {
                // Some local servers omit the id; the agent loop needs one to
                // pair the result with the call.
                id: if call.id.is_empty() {
                    format!("call_{index}_{}", uuid::Uuid::new_v4().simple())
                } else {
                    call.id
                },
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: call.name,
                    arguments: if call.arguments.is_empty() {
                        "{}".to_string()
                    } else {
                        call.arguments
                    },
                },
            })
            .collect();

        CompletedChat {
            content: self.content,
            thinking: self.thinking,
            thinking_signature: None,
            tool_calls,
            usage: self.usage,
            finish_reason: self.finish_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(lines: &[&str]) -> CompletedChat {
        let mut stream = Box::<OpenAiStream>::default();
        for line in lines {
            stream.push_line(line).expect("line accepted");
        }
        stream.finish()
    }

    /// Feeds a recorded SSE body through the line buffer, as the client does.
    fn replay(recording: &str) -> CompletedChat {
        replay_with_deltas(recording).0
    }

    /// [`replay`], also keeping every delta the lines produced, in order.
    fn replay_with_deltas(recording: &str) -> (CompletedChat, Vec<StreamDelta>) {
        let mut lines = LineBuffer::default();
        let mut stream = Box::<OpenAiStream>::default();
        let mut deltas = Vec::new();
        for line in lines.push(recording.as_bytes()) {
            let step = stream.push_line(&line).expect("line accepted");
            deltas.extend(step.deltas);
            if step.done {
                break;
            }
        }
        (stream.finish(), deltas)
    }

    #[test]
    fn deltas_arrive_in_order_and_add_up_to_the_final_answer() {
        let (chat, deltas) =
            replay_with_deltas(include_str!("../tests/fixtures/vllm_reasoning.sse"));

        let thinking: String = deltas
            .iter()
            .filter_map(|d| d.thinking.as_deref())
            .collect();
        let content: String = deltas.iter().filter_map(|d| d.content.as_deref()).collect();
        assert_eq!(thinking, chat.thinking);
        assert_eq!(content, chat.content);
        // Reasoning streams before the answer starts.
        let first_thinking = deltas.iter().position(|d| d.thinking.is_some()).unwrap();
        let first_content = deltas.iter().position(|d| d.content.is_some()).unwrap();
        assert!(first_thinking < first_content);
        // The usage chunk and the finish reason are reported once each, last.
        assert_eq!(deltas.iter().filter(|d| d.usage.is_some()).count(), 1);
        assert_eq!(
            deltas
                .iter()
                .filter_map(|d| d.finish_reason.as_deref())
                .collect::<Vec<_>>(),
            vec![chat.finish_reason.as_deref().unwrap()]
        );
    }

    #[test]
    fn tool_call_fragments_add_up_to_the_final_calls() {
        let (chat, deltas) =
            replay_with_deltas(include_str!("../tests/fixtures/openai_tool_calls.sse"));

        let fragments: Vec<&ToolCallDelta> = deltas.iter().flat_map(|d| &d.tool_calls).collect();
        for (index, call) in chat.tool_calls.iter().enumerate() {
            let of_call: Vec<_> = fragments
                .iter()
                .filter(|f| f.index == index as u64)
                .collect();
            // The first fragment names the call; the rest only add arguments.
            assert_eq!(of_call[0].id.as_deref(), Some(call.id.as_str()));
            assert_eq!(
                of_call[0].function.name.as_deref(),
                Some(call.function.name.as_str())
            );
            let arguments: String = of_call
                .iter()
                .map(|f| f.function.arguments.as_str())
                .collect();
            assert_eq!(arguments, call.function.arguments);
        }
    }

    #[test]
    fn a_line_with_nothing_new_produces_no_delta() {
        let mut stream = OpenAiStream::default();
        for line in [
            "",
            ": keep-alive",
            "event: ping",
            r#"data: {"choices":[{"delta":{}}]}"#,
        ] {
            assert!(stream.push_line(line).unwrap().deltas.is_empty(), "{line}");
        }
    }

    #[test]
    fn a_delta_serializes_in_the_shape_the_frontend_stream_parser_reads() {
        let delta = StreamDelta {
            thinking: Some("hm".to_string()),
            thinking_signature: Some("sig".to_string()),
            tool_calls: vec![ToolCallDelta {
                index: 1,
                id: Some("c1".to_string()),
                function: crate::types::ToolCallFunctionDelta {
                    name: Some("t".to_string()),
                    arguments: "{".to_string(),
                },
            }],
            ..StreamDelta::default()
        };
        assert_eq!(
            serde_json::to_value(&delta).unwrap(),
            serde_json::json!({
                "thinking": "hm",
                "thinkingSignature": "sig",
                "tool_calls": [{ "index": 1, "id": "c1", "function": { "name": "t", "arguments": "{" } }],
            })
        );
        let back: StreamDelta =
            serde_json::from_value(serde_json::to_value(&delta).unwrap()).unwrap();
        assert_eq!(back, delta);
    }

    #[test]
    fn lines_survive_being_split_across_chunks() {
        let mut buffer = LineBuffer::default();
        assert!(buffer.push(b"data: {\"a\"").is_empty());
        assert_eq!(
            buffer.push(b":1}\r\n\ndata: x"),
            vec!["data: {\"a\":1}", ""]
        );
        assert_eq!(buffer.finish().as_deref(), Some("data: x"));
    }

    #[test]
    fn recorded_openai_text_stream_keeps_text_and_usage_details() {
        let chat = replay(include_str!("../tests/fixtures/openai_text.sse"));

        assert_eq!(chat.content, "Hello! How can I help you today?");
        assert_eq!(chat.finish_reason.as_deref(), Some("stop"));
        let usage = chat.usage.expect("usage chunk");
        assert_eq!(usage.prompt_tokens, 1_210);
        assert_eq!(usage.completion_tokens, 10);
        assert_eq!(usage.total_tokens, 1_220);
        assert_eq!(usage.cached_prompt_tokens, Some(1_152));
        assert_eq!(usage.details.and_then(|d| d.reasoning_tokens), Some(0));
        assert!(chat.tool_calls.is_empty());
    }

    #[test]
    fn recorded_parallel_tool_calls_are_joined_per_index() {
        let chat = replay(include_str!("../tests/fixtures/openai_tool_calls.sse"));

        assert_eq!(chat.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(chat.tool_calls.len(), 2);
        assert_eq!(chat.tool_calls[0].id, "call_Qx7mJ2");
        assert_eq!(chat.tool_calls[0].function.name, "workspace__readFile");
        assert_eq!(
            chat.tool_calls[0].function.arguments,
            r#"{"path":"notes/todo.md"}"#
        );
        assert_eq!(chat.tool_calls[1].id, "call_Zr81kP");
        assert_eq!(chat.tool_calls[1].function.name, "workspace__listDirectory");
        assert_eq!(chat.tool_calls[1].function.arguments, r#"{"path":"."}"#);
    }

    #[test]
    fn recorded_vllm_reasoning_stream_collects_thinking_apart_from_the_answer() {
        let chat = replay(include_str!("../tests/fixtures/vllm_reasoning.sse"));

        assert_eq!(chat.thinking, "The user wants 2+2. That is 4.");
        assert_eq!(chat.content, "2 + 2 = 4.");
        let usage = chat.usage.expect("usage chunk");
        assert_eq!(usage.prompt_tokens, 31);
        // A stream without `prompt_tokens_details` reports no cache hit.
        assert_eq!(usage.cached_prompt_tokens, None);
    }

    #[test]
    fn deepseek_style_cache_hits_are_read_too() {
        let chat = feed(&[
            r#"data: {"choices":[],"usage":{"prompt_tokens":40,"completion_tokens":2,"total_tokens":42,"prompt_cache_hit_tokens":32}}"#,
        ]);
        assert_eq!(chat.usage.unwrap().cached_prompt_tokens, Some(32));
    }

    #[test]
    fn a_missing_tool_call_id_gets_a_generated_one_and_empty_arguments_become_an_object() {
        let chat = feed(&[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"t","arguments":""}}]}}]}"#,
        ]);
        assert!(chat.tool_calls[0].id.starts_with("call_0_"));
        // No arguments at all still has to be valid JSON for the tool runner.
        assert_eq!(chat.tool_calls[0].function.arguments, "{}");
    }

    #[test]
    fn an_error_chunk_or_bad_json_fails_the_stream() {
        let mut stream = OpenAiStream::default();
        let error = stream
            .push_line(r#"data: {"error":{"message":"context length exceeded"}}"#)
            .unwrap_err();
        assert!(error.contains("context length exceeded"), "{error}");

        let mut stream = OpenAiStream::default();
        assert!(stream.push_line("data: {not json").is_err());
    }
}
