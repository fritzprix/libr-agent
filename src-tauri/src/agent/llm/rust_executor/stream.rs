//! Accumulates an OpenAI chat-completions SSE stream into one assistant turn.

use crate::agent::types::{ToolCall, ToolCallFunction};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Splits a byte stream into complete lines; a line can span network chunks.
#[derive(Default)]
pub(super) struct LineBuffer {
    pending: Vec<u8>,
}

impl LineBuffer {
    pub(super) fn push(&mut self, chunk: &[u8]) -> Vec<String> {
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
    pub(super) fn finish(self) -> Option<String> {
        (!self.pending.is_empty()).then(|| String::from_utf8_lossy(&self.pending).to_string())
    }
}

#[derive(Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// What one completed stream produced.
#[derive(Debug, Default, PartialEq)]
pub(super) struct CompletedChat {
    pub(super) content: String,
    pub(super) thinking: String,
    pub(super) tool_calls: Vec<ToolCall>,
    /// `{ promptTokens, completionTokens, totalTokens }`, as the frontend stores it.
    pub(super) usage: Option<Value>,
    pub(super) finish_reason: Option<String>,
}

#[derive(Default)]
pub(super) struct ChatStreamAccumulator {
    content: String,
    thinking: String,
    tool_calls: BTreeMap<u64, PartialToolCall>,
    usage: Option<Value>,
    finish_reason: Option<String>,
    done: bool,
}

impl ChatStreamAccumulator {
    /// Feed one SSE line. Returns `Ok(true)` once the stream said `[DONE]`.
    pub(super) fn push_line(&mut self, line: &str) -> Result<bool, String> {
        let Some(data) = line.strip_prefix("data:") else {
            // Comments, `event:` lines, and blank separators carry nothing here.
            return Ok(self.done);
        };
        let data = data.trim();
        if data.is_empty() {
            return Ok(self.done);
        }
        if data == "[DONE]" {
            self.done = true;
            return Ok(true);
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

        if let Some(usage) = chunk.get("usage").filter(|usage| usage.is_object()) {
            self.usage = Some(json!({
                "promptTokens": usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
                "completionTokens": usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
                "totalTokens": usage.get("total_tokens").and_then(Value::as_u64).unwrap_or(0),
            }));
        }

        let Some(choice) = chunk.get("choices").and_then(|choices| choices.get(0)) else {
            return Ok(false);
        };
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.to_string());
        }
        let Some(delta) = choice.get("delta") else {
            return Ok(false);
        };

        // vLLM-served Qwen models use `reasoning`; others use `reasoning_content`.
        for key in ["reasoning_content", "reasoning"] {
            if let Some(text) = delta.get(key).and_then(Value::as_str) {
                if !text.is_empty() {
                    self.thinking.push_str(text);
                    break;
                }
            }
        }
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            self.content.push_str(text);
        }
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let Some(index) = call.get("index").and_then(Value::as_u64) else {
                    continue;
                };
                let partial = self.tool_calls.entry(index).or_default();
                if let Some(id) = call.get("id").and_then(Value::as_str) {
                    if !id.is_empty() {
                        partial.id = id.to_string();
                    }
                }
                if let Some(function) = call.get("function") {
                    if let Some(name) = function.get("name").and_then(Value::as_str) {
                        partial.name.push_str(name);
                    }
                    if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                        partial.arguments.push_str(arguments);
                    }
                }
            }
        }

        Ok(false)
    }

    pub(super) fn finish(self) -> CompletedChat {
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
        let mut accumulator = ChatStreamAccumulator::default();
        for line in lines {
            accumulator.push_line(line).expect("line accepted");
        }
        accumulator.finish()
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
    fn text_reasoning_and_usage_are_collected() {
        let chat = feed(&[
            r#"data: {"choices":[{"delta":{"reasoning_content":"think "}}]}"#,
            r#"data: {"choices":[{"delta":{"reasoning":"more"}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#,
            ": keep-alive",
            r#"data: {"choices":[{"delta":{"content":"lo"},"finish_reason":"stop"}]}"#,
            r#"data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":3,"total_tokens":15}}"#,
            "data: [DONE]",
        ]);

        assert_eq!(chat.content, "Hello");
        assert_eq!(chat.thinking, "think more");
        assert_eq!(chat.finish_reason.as_deref(), Some("stop"));
        assert_eq!(
            chat.usage,
            Some(json!({ "promptTokens": 12, "completionTokens": 3, "totalTokens": 15 }))
        );
        assert!(chat.tool_calls.is_empty());
    }

    #[test]
    fn tool_call_arguments_are_joined_per_index() {
        let chat = feed(&[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"workspace__readFile","arguments":"{\"pa"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_b","function":{"name":"workspace__listDirectory","arguments":""}}]}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\":\"a.txt\"}"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        ]);

        assert_eq!(chat.tool_calls.len(), 2);
        assert_eq!(chat.tool_calls[0].id, "call_a");
        assert_eq!(chat.tool_calls[0].function.name, "workspace__readFile");
        assert_eq!(chat.tool_calls[0].function.arguments, r#"{"path":"a.txt"}"#);
        assert_eq!(chat.tool_calls[1].id, "call_b");
        // No arguments at all still has to be valid JSON for the tool runner.
        assert_eq!(chat.tool_calls[1].function.arguments, "{}");
    }

    #[test]
    fn a_missing_tool_call_id_gets_a_generated_one() {
        let chat = feed(&[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"t","arguments":"{}"}}]}}]}"#,
        ]);
        assert!(chat.tool_calls[0].id.starts_with("call_0_"));
    }

    #[test]
    fn an_error_chunk_or_bad_json_fails_the_stream() {
        let mut accumulator = ChatStreamAccumulator::default();
        let error = accumulator
            .push_line(r#"data: {"error":{"message":"context length exceeded"}}"#)
            .unwrap_err();
        assert!(error.contains("context length exceeded"), "{error}");

        let mut accumulator = ChatStreamAccumulator::default();
        assert!(accumulator.push_line("data: {not json").is_err());
    }
}
