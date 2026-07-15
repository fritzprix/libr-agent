use crate::agent::types::ToolCall;
use crate::agent::AgentConfig;
use crate::models::chat::Message;
use crate::repositories::settings_repository::SettingsRepository;
#[derive(Debug, PartialEq)]
pub enum CircuitBreakerAction {
    NaturalRecoveryError {
        count: usize,
        tool_name: String,
        args: String,
    },
    NaturalRecoverySuccess {
        count: usize,
        tool_name: String,
        args: String,
    },
    HardBreak {
        count: usize,
        tool_name: String,
        args: String,
    },
}

pub(crate) fn is_builtin_alias_enabled(agent_config: &AgentConfig, alias: &str) -> bool {
    crate::agent::tools::is_builtin_service_alias_enabled(agent_config, alias)
}

fn is_tool_error_message(message: &Message) -> bool {
    if message.role != "tool" {
        return false;
    }

    let metadata_tool_error = message
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolError"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    if metadata_tool_error {
        return true;
    }

    message.content.iter().any(|content| {
        matches!(
            content,
            crate::mcp::types::MCPContent::Text {
                is_error: Some(true),
                ..
            }
        )
    })
}

/// Build signature_by_id lookup map from message history in a single pass.
pub fn build_tool_call_indices(messages: &[Message]) -> std::collections::HashMap<String, String> {
    let mut call_signature_by_id = std::collections::HashMap::new();

    for message in messages {
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                call_signature_by_id.insert(
                    tool_call.id.clone(),
                    format!(
                        "{}:{}",
                        tool_call.function.name, tool_call.function.arguments
                    ),
                );
            }
        }
    }

    call_signature_by_id
}

pub(crate) async fn load_loop_prevention_threshold() -> usize {
    let default_threshold = 3;
    let Some(settings_repo) = crate::state::try_get_settings_repository() else {
        return default_threshold;
    };

    let val = match settings_repo.get("advancedSettings").await {
        Ok(Some(model)) => match serde_json::from_str::<serde_json::Value>(&model.value) {
            Ok(json) => json
                .get("loopPreventionThreshold")
                .and_then(|value| value.as_u64())
                .map(|v| v as usize)
                .unwrap_or(default_threshold),
            Err(_) => default_threshold,
        },
        _ => default_threshold,
    };
    val.clamp(2, 20)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepeatedOutcome {
    Success { signature: String },
    Error { signature: String },
}

fn normalize_text_signature(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn build_tool_result_signature(message: &Message) -> String {
    let text_signature = message
        .content
        .iter()
        .filter_map(|content| match content {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                let normalized = normalize_text_signature(text);
                if normalized.is_empty() {
                    None
                } else {
                    Some(normalized)
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if text_signature.is_empty() {
        if is_tool_error_message(message) {
            "__tool_error__".to_string()
        } else {
            "__tool_success__".to_string()
        }
    } else {
        text_signature
    }
}

fn is_loop_prevention_message(message: &Message) -> bool {
    if message
        .metadata
        .as_ref()
        .and_then(|metadata| {
            metadata
                .get("loopPrevention")
                .or_else(|| {
                    metadata
                        .get("structuredContent")
                        .and_then(|value| value.get("loopPrevention"))
                })
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(false)
    {
        return true;
    }

    message.content.iter().any(|content| {
        matches!(
            content,
            crate::mcp::types::MCPContent::Text { text, .. }
                if text.starts_with("Loop prevention:")
        )
    })
}

/// Count trailing tool results whose call signature matches `matcher`.
///
/// A different tool call (name/args) ends the streak and resets the counter.
/// Outcome text differences do NOT reset the counter — once the agent is looping the
/// same call, further repeats stay blocked until a different tool/args appears.
/// Loop-prevention short-circuit results also keep the streak (they must not look like
/// a “new outcome” that clears the counter).
fn count_consecutive_identical_call_outcomes<F>(
    messages: &[Message],
    matcher: F,
) -> Option<(usize, RepeatedOutcome)>
where
    F: Fn(&str) -> bool,
{
    let mut consecutive_matches = 0;
    let mut saw_tool_result = false;
    let mut repeated_outcome: Option<RepeatedOutcome> = None;

    for message in messages.iter().rev() {
        match message.role.as_str() {
            "tool" => {
                saw_tool_result = true;
                let Some(tool_call_id) = message.tool_call_id.as_deref() else {
                    break;
                };

                if !matcher(tool_call_id) {
                    break;
                }

                consecutive_matches += 1;

                if is_loop_prevention_message(message) {
                    continue;
                }

                if repeated_outcome.is_none() {
                    let signature = build_tool_result_signature(message);
                    repeated_outcome = Some(if is_tool_error_message(message) {
                        RepeatedOutcome::Error { signature }
                    } else {
                        RepeatedOutcome::Success { signature }
                    });
                }
            }
            "assistant" => {}
            _ => {
                if saw_tool_result {
                    break;
                }
            }
        }
    }

    if consecutive_matches == 0 {
        return None;
    }

    let outcome = repeated_outcome.unwrap_or_else(|| RepeatedOutcome::Error {
        signature: "__loop_prevention__".to_string(),
    });
    Some((consecutive_matches, outcome))
}

pub fn evaluate_circuit_breaker_action(
    messages: &[Message],
    tool_call: &ToolCall,
    call_signature_by_id: &std::collections::HashMap<String, String>,
    threshold: usize,
) -> Option<CircuitBreakerAction> {
    let tool_name = &tool_call.function.name;
    let args = &tool_call.function.arguments;

    if tool_name == "ui__circuitBreak"
        || tool_name == "scratchpad__think"
        || tool_name == "planning__reflect"
    {
        return None;
    }

    let current_signature = format!("{}:{}", tool_name, args);
    if let Some((consecutive_identical_signature, outcome)) =
        count_consecutive_identical_call_outcomes(messages, |tool_call_id| {
            call_signature_by_id.get(tool_call_id) == Some(&current_signature)
        })
    {
        let total_count = consecutive_identical_signature + 1;

        if total_count > threshold {
            return Some(CircuitBreakerAction::HardBreak {
                count: total_count,
                tool_name: tool_name.clone(),
                args: args.clone(),
            });
        } else if total_count == threshold {
            return Some(match outcome {
                RepeatedOutcome::Error { .. } => CircuitBreakerAction::NaturalRecoveryError {
                    count: total_count,
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                },
                RepeatedOutcome::Success { .. } => CircuitBreakerAction::NaturalRecoverySuccess {
                    count: total_count,
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                },
            });
        }
    }

    None
}
