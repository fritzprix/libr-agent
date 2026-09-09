//! ATIF-v1.7 trajectory builder aligned with Harbor `build_atif_trajectory`.

use crate::agent::types::ToolCall as LibrAgentToolCall;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Map, Value};

const SCHEMA_VERSION: &str = "ATIF-v1.7";
const EMPTY_TRAJECTORY_MESSAGE: &str = "(no LibrAgent messages harvested for ATIF trajectory)";

#[derive(Debug, Clone)]
pub struct AtifAgentMetadata {
    pub name: String,
    pub version: String,
    pub model_name: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtifTrajectory {
    pub schema_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub agent: AtifAgent,
    pub steps: Vec<AtifStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_metrics: Option<AtifFinalMetrics>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtifAgent {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtifStep {
    pub step_id: usize,
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<AtifToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<AtifObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<AtifMetrics>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtifToolCall {
    pub tool_call_id: String,
    pub function_name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtifObservation {
    pub results: Vec<AtifObservationResult>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtifObservationResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_call_id: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtifMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtifFinalMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_prompt_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_completion_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cached_tokens: Option<i64>,
    pub total_steps: usize,
}

pub(super) struct AtifTrajectoryBuilder {
    metadata: AtifAgentMetadata,
    steps: Vec<AtifStep>,
    pending_by_id: IndexMap<String, Vec<AtifObservationResult>>,
    n_input: i64,
    n_output: i64,
    n_cache: i64,
    has_usage: bool,
}

impl AtifTrajectoryBuilder {
    pub(super) fn new(metadata: AtifAgentMetadata) -> Self {
        Self {
            metadata,
            steps: Vec::new(),
            pending_by_id: IndexMap::new(),
            n_input: 0,
            n_output: 0,
            n_cache: 0,
            has_usage: false,
        }
    }

    pub(super) fn push(&mut self, message: &Message) {
        self.accumulate_usage(message.usage.as_ref());
        let role = message.role.to_ascii_lowercase();
        match role.as_str() {
            "user" => {
                let text = assistant_message_text(message);
                self.steps.push(AtifStep {
                    step_id: self.steps.len() + 1,
                    source: "user".to_string(),
                    message: if text.is_empty() {
                        "(empty user message)".to_string()
                    } else {
                        text
                    },
                    reasoning_content: None,
                    tool_calls: None,
                    observation: None,
                    metrics: None,
                });
            }
            "system" => {
                let text = assistant_message_text(message);
                self.steps.push(AtifStep {
                    step_id: self.steps.len() + 1,
                    source: "system".to_string(),
                    message: if text.is_empty() {
                        "(empty system message)".to_string()
                    } else {
                        text
                    },
                    reasoning_content: None,
                    tool_calls: None,
                    observation: None,
                    metrics: None,
                });
            }
            "tool" => attach_tool_observation(&mut self.steps, message, &mut self.pending_by_id),
            "assistant" => {
                let reasoning = assistant_reasoning(message);
                let tool_calls = libragent_tool_calls(message);
                let mut message_text = assistant_message_text(message);
                if message_text.is_empty() {
                    message_text =
                        if let Some(first) = tool_calls.as_ref().and_then(|calls| calls.first()) {
                            format!("(assistant tool call: {})", first.function_name)
                        } else if reasoning.is_some() {
                            "(assistant reasoning only)".to_string()
                        } else {
                            "(empty assistant message)".to_string()
                        };
                }

                let mut step = AtifStep {
                    step_id: self.steps.len() + 1,
                    source: "agent".to_string(),
                    message: message_text,
                    reasoning_content: reasoning,
                    tool_calls,
                    observation: None,
                    metrics: step_metrics_from_usage(message.usage.as_ref()),
                };
                consume_pending_observations(&mut step, &mut self.pending_by_id);
                self.steps.push(step);
            }
            _ => {}
        }
    }

    fn accumulate_usage(&mut self, usage: Option<&Value>) {
        let Some(usage) = usage else {
            return;
        };
        let prompt = usage_token(usage, &["promptTokens", "prompt_tokens", "input_tokens"]);
        let completion = usage_token(
            usage,
            &["completionTokens", "completion_tokens", "output_tokens"],
        );
        let cached = usage_token(
            usage,
            &[
                "cachedPromptTokens",
                "cached_prompt_tokens",
                "cache_read_input_tokens",
                "cached_tokens",
            ],
        );
        if prompt.is_none() && completion.is_none() && cached.is_none() {
            return;
        }
        self.has_usage = true;
        self.n_input += prompt.unwrap_or(0);
        self.n_output += completion.unwrap_or(0);
        self.n_cache += cached.unwrap_or(0);
    }

    pub(super) fn finish(mut self) -> AtifTrajectory {
        if !self.pending_by_id.is_empty() {
            let orphan_results: Vec<AtifObservationResult> = self
                .pending_by_id
                .into_values()
                .flatten()
                .map(|result| AtifObservationResult {
                    source_call_id: None,
                    content: result.content,
                })
                .collect();
            let agent_index = self.steps.iter().rposition(|step| step.source == "agent");
            let agent_index = match agent_index {
                Some(index) => index,
                None => {
                    self.steps.push(AtifStep {
                        step_id: self.steps.len() + 1,
                        source: "agent".to_string(),
                        message: "(orphaned LibrAgent tool results)".to_string(),
                        reasoning_content: None,
                        tool_calls: None,
                        observation: None,
                        metrics: None,
                    });
                    self.steps.len() - 1
                }
            };
            for result in orphan_results {
                append_observation_result(&mut self.steps[agent_index], result);
            }
        }

        if self.steps.is_empty() {
            self.steps.push(AtifStep {
                step_id: 1,
                source: "agent".to_string(),
                message: EMPTY_TRAJECTORY_MESSAGE.to_string(),
                reasoning_content: None,
                tool_calls: None,
                observation: None,
                metrics: None,
            });
        }

        let (total_prompt_tokens, total_completion_tokens, total_cached_tokens) = if self.has_usage
        {
            (Some(self.n_input), Some(self.n_output), Some(self.n_cache))
        } else {
            (None, None, None)
        };

        AtifTrajectory {
            schema_version: SCHEMA_VERSION.to_string(),
            session_id: self.metadata.session_id,
            agent: AtifAgent {
                name: self.metadata.name,
                version: self.metadata.version,
                model_name: self.metadata.model_name,
            },
            final_metrics: Some(AtifFinalMetrics {
                total_prompt_tokens,
                total_completion_tokens,
                total_cached_tokens,
                total_steps: self.steps.len(),
            }),
            steps: self.steps,
        }
    }
}

/// Convert persisted LibrAgent messages into an ATIF-v1.7 trajectory.
pub fn build_atif_trajectory(messages: &[Message], metadata: AtifAgentMetadata) -> AtifTrajectory {
    let mut builder = AtifTrajectoryBuilder::new(metadata);
    for message in messages {
        builder.push(message);
    }
    builder.finish()
}

fn message_text_parts(message: &Message, part_types: &[&str]) -> Vec<String> {
    let mut texts = Vec::new();
    for part in &message.content {
        match part {
            MCPContent::Text { text } if part_types.contains(&"text") => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    texts.push(trimmed.to_string());
                }
            }
            MCPContent::Thinking { thinking, .. } if part_types.contains(&"thinking") => {
                let trimmed = thinking.trim();
                if !trimmed.is_empty() {
                    texts.push(trimmed.to_string());
                }
            }
            _ => {}
        }
    }
    texts
}

fn assistant_message_text(message: &Message) -> String {
    message_text_parts(message, &["text"]).join("\n")
}

fn assistant_reasoning(message: &Message) -> Option<String> {
    if let Some(thinking) = message.thinking.as_deref() {
        let trimmed = thinking.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let parts = message_text_parts(message, &["thinking"]);
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn parse_tool_arguments(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Value::Object(Map::new());
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Object(map)) => Value::Object(map),
        Ok(other) => Value::Object(Map::from_iter([("value".to_string(), other)])),
        Err(_) => Value::Object(Map::from_iter([(
            "raw".to_string(),
            Value::String(raw.to_string()),
        )])),
    }
}

fn libragent_tool_calls(message: &Message) -> Option<Vec<AtifToolCall>> {
    let mut converted = Vec::new();
    if let Some(calls) = message.tool_calls.as_ref() {
        for (index, call) in calls.iter().enumerate() {
            converted.push(convert_tool_call(call, index));
        }
    }
    if converted.is_empty() {
        for (index, part) in message.content.iter().enumerate() {
            if let MCPContent::ToolCall {
                id,
                name,
                arguments,
            } = part
            {
                converted.push(AtifToolCall {
                    tool_call_id: if id.trim().is_empty() {
                        format!("tool_call_{}", index + 1)
                    } else {
                        id.clone()
                    },
                    function_name: if name.trim().is_empty() {
                        format!("unknown_tool_{}", index + 1)
                    } else {
                        name.clone()
                    },
                    arguments: parse_tool_arguments(arguments),
                });
            }
        }
    }
    if converted.is_empty() {
        None
    } else {
        Some(converted)
    }
}

fn convert_tool_call(call: &LibrAgentToolCall, index: usize) -> AtifToolCall {
    let function_name = if call.function.name.trim().is_empty() {
        format!("unknown_tool_{}", index + 1)
    } else {
        call.function.name.trim().to_string()
    };
    let tool_call_id = if call.id.trim().is_empty() {
        format!("tool_call_{}", index + 1)
    } else {
        call.id.trim().to_string()
    };
    AtifToolCall {
        tool_call_id,
        function_name,
        arguments: parse_tool_arguments(&call.function.arguments),
    }
}

fn usage_token(usage: &Value, keys: &[&str]) -> Option<i64> {
    let object = usage.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key) {
            if let Some(number) = json_to_i64(value) {
                return Some(number);
            }
        }
    }
    None
}

fn json_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|n| n as i64))
        .or_else(|| value.as_f64().map(|n| n as i64))
}

fn step_metrics_from_usage(usage: Option<&Value>) -> Option<AtifMetrics> {
    let usage = usage?;
    let prompt = usage_token(usage, &["promptTokens", "prompt_tokens", "input_tokens"]);
    let completion = usage_token(
        usage,
        &["completionTokens", "completion_tokens", "output_tokens"],
    );
    let cached = usage_token(
        usage,
        &[
            "cachedPromptTokens",
            "cached_prompt_tokens",
            "cache_read_input_tokens",
            "cached_tokens",
        ],
    );
    if prompt.is_none() && completion.is_none() && cached.is_none() {
        return None;
    }
    Some(AtifMetrics {
        prompt_tokens: prompt,
        completion_tokens: completion,
        cached_tokens: cached,
    })
}

fn media_placeholder(kind: &str, mime_type: &str, uri: Option<&str>) -> String {
    match uri.map(str::trim).filter(|value| !value.is_empty()) {
        Some(uri) => format!("[{kind}: {mime_type} - {uri}]"),
        None => format!("[{kind}: {mime_type}]"),
    }
}

fn observation_content_part(part: &MCPContent) -> Option<String> {
    match part {
        MCPContent::Text { text } => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        MCPContent::Image { mime_type, uri, .. } => {
            Some(media_placeholder("Image", mime_type, uri.as_deref()))
        }
        MCPContent::Audio { mime_type, uri, .. } => {
            Some(media_placeholder("Audio", mime_type, uri.as_deref()))
        }
        MCPContent::Resource { resource, .. } => {
            let mime_type = resource
                .get("mimeType")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let uri = resource.get("uri").and_then(Value::as_str);
            Some(match uri {
                Some(uri) => format!("[UI Resource: {mime_type} - {uri}]"),
                None => format!("[UI Resource: {mime_type}]"),
            })
        }
        MCPContent::Thinking { thinking, .. } => {
            let trimmed = thinking.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        MCPContent::ToolCall { name, .. } => Some(format!("[Tool call: {name}]")),
    }
}

fn tool_observation_content(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(observation_content_part)
        .collect::<Vec<_>>()
        .join("\n")
}

fn append_observation_result(step: &mut AtifStep, result: AtifObservationResult) {
    match step.observation.as_mut() {
        Some(observation) => observation.results.push(result),
        None => {
            step.observation = Some(AtifObservation {
                results: vec![result],
            });
        }
    }
}

fn find_agent_step_for_tool_call<'a>(
    steps: &'a mut [AtifStep],
    tool_call_id: &str,
) -> Option<&'a mut AtifStep> {
    steps.iter_mut().rev().find(|step| {
        step.source == "agent"
            && step
                .tool_calls
                .as_ref()
                .is_some_and(|calls| calls.iter().any(|call| call.tool_call_id == tool_call_id))
    })
}

fn attach_tool_observation(
    steps: &mut [AtifStep],
    message: &Message,
    pending_by_id: &mut IndexMap<String, Vec<AtifObservationResult>>,
) {
    let source_call_id = message
        .tool_call_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned);
    let result = AtifObservationResult {
        source_call_id: source_call_id.clone(),
        content: tool_observation_content(message),
    };

    if let Some(call_id) = source_call_id {
        if let Some(matched) = find_agent_step_for_tool_call(steps, &call_id) {
            append_observation_result(matched, result);
            return;
        }
        pending_by_id.entry(call_id).or_default().push(result);
        return;
    }

    if let Some(agent_step) = steps.iter_mut().rev().find(|step| step.source == "agent") {
        append_observation_result(agent_step, result);
    }
}

fn consume_pending_observations(
    step: &mut AtifStep,
    pending_by_id: &mut IndexMap<String, Vec<AtifObservationResult>>,
) {
    let Some(tool_calls) = step.tool_calls.as_ref() else {
        return;
    };
    let mut results = Vec::new();
    for tool_call in tool_calls {
        if let Some(pending) = pending_by_id.shift_remove(&tool_call.tool_call_id) {
            results.extend(pending);
        }
    }
    if results.is_empty() {
        return;
    }
    match step.observation.as_mut() {
        Some(observation) => observation.results.extend(results),
        None => step.observation = Some(AtifObservation { results }),
    }
}
