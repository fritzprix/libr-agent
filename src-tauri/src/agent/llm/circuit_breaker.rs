use crate::agent::types::ToolCall;
use crate::models::chat::Message;
use std::collections::HashMap;

pub(crate) fn is_builtin_alias_enabled(agent_config: Option<&str>, alias: &str) -> bool {
    let Some(config_str) = agent_config else {
        return true;
    };

    let Ok(parsed_config) = crate::agent::AgentConfig::from_json(config_str) else {
        return true;
    };

    crate::agent::tools::is_builtin_service_alias_enabled(&parsed_config, alias)
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

/// Build (name_by_id, signature_by_id) lookup maps from message history in a single pass.
pub(crate) fn build_tool_call_indices(
    messages: &[Message],
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut call_name_by_id = HashMap::new();
    let mut call_signature_by_id = HashMap::new();

    for message in messages {
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                call_name_by_id.insert(tool_call.id.clone(), tool_call.function.name.clone());
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

    (call_name_by_id, call_signature_by_id)
}

/// Count consecutive failed tool calls matching a given predicate.
///
/// Iterates backwards through messages counting consecutive tool error responses
/// that match the given predicate. Stops at the first successful response or
/// non-matching error.
fn count_consecutive_failed_calls<F>(messages: &[Message], matcher: F) -> usize
where
    F: Fn(&str) -> bool,
{
    let mut consecutive_failures = 0;
    let mut saw_tool_result = false;

    for message in messages.iter().rev() {
        match message.role.as_str() {
            "tool" => {
                saw_tool_result = true;

                if !is_tool_error_message(message) {
                    break;
                }

                let Some(tool_call_id) = message.tool_call_id.as_deref() else {
                    break;
                };

                if matcher(tool_call_id) {
                    consecutive_failures += 1;
                } else {
                    break;
                }
            }
            "assistant" => {
                // Assistant messages often sit between tool results; skip them.
            }
            _ => {
                if saw_tool_result {
                    break;
                }
            }
        }
    }

    consecutive_failures
}

pub(crate) fn evaluate_circuit_breaker_count(
    messages: &[Message],
    tool_call: &ToolCall,
    call_name_by_id: &HashMap<String, String>,
    call_signature_by_id: &HashMap<String, String>,
) -> Option<usize> {
    let tool_name = &tool_call.function.name;
    let args = &tool_call.function.arguments;

    if tool_name == "ui__circuitBreak" {
        return None;
    }

    // Check for consecutive failures of the same tool name
    let consecutive_failed_same_tool = count_consecutive_failed_calls(messages, |tool_call_id| {
        call_name_by_id.get(tool_call_id) == Some(tool_name)
    });
    if consecutive_failed_same_tool >= 2 {
        return Some(consecutive_failed_same_tool + 1);
    }

    // Check for consecutive failures of the same tool signature (name + args)
    let current_signature = format!("{}:{}", tool_name, args);
    let consecutive_failed_same_signature =
        count_consecutive_failed_calls(messages, |tool_call_id| {
            call_signature_by_id.get(tool_call_id) == Some(&current_signature)
        });

    if consecutive_failed_same_signature >= 2 {
        return Some(consecutive_failed_same_signature + 1);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::ToolCallFunction;
    use crate::mcp::types::MCPContent;

    fn test_message(
        id: &str,
        role: &str,
        tool_calls: Option<Vec<ToolCall>>,
        tool_call_id: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Message {
        Message {
            id: id.to_string(),
            session_id: "session-test".to_string(),
            role: role.to_string(),
            content: vec![MCPContent::Text {
                text: "ok".to_string(),
                is_error: None,
            }],
            tool_calls,
            tool_call_id: tool_call_id.map(str::to_string),
            is_streaming: Some(false),
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            created_at: 0,
            updated_at: 0,
            source: None,
            error: None,
            metadata,
        }
    }

    fn test_tool_call(id: &str, name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
        }
    }

    #[test]
    fn circuit_breaker_triggers_on_consecutive_failed_same_tool_with_different_args() {
        let messages = vec![
            test_message(
                "assistant-1",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-1",
                    "planning__clearScratchpad",
                    r#"{"id":191}"#,
                )]),
                None,
                None,
            ),
            test_message(
                "tool-1",
                "tool",
                None,
                Some("tc-1"),
                Some(serde_json::json!({ "toolError": true })),
            ),
            test_message(
                "assistant-2",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-2",
                    "planning__clearScratchpad",
                    r#"{"id":192}"#,
                )]),
                None,
                None,
            ),
            test_message(
                "tool-2",
                "tool",
                None,
                Some("tc-2"),
                Some(serde_json::json!({ "toolError": true })),
            ),
        ];

        let (call_name_by_id, call_signature_by_id) = build_tool_call_indices(&messages);
        let current_batch = [test_tool_call(
            "tc-3",
            "planning__clearScratchpad",
            r#"{"id":193}"#,
        )];

        let trigger_count = evaluate_circuit_breaker_count(
            &messages,
            &current_batch[0],
            &call_name_by_id,
            &call_signature_by_id,
        );

        assert_eq!(trigger_count, Some(3));
    }

    #[test]
    fn circuit_breaker_triggers_on_failed_signature_repetition() {
        let repeated_args = r#"{"id":7}"#;
        let messages = vec![
            test_message(
                "assistant-1",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-1",
                    "planning__clearScratchpad",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message(
                "assistant-2",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-2",
                    "planning__clearScratchpad",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message(
                "tool-1",
                "tool",
                None,
                Some("tc-1"),
                Some(serde_json::json!({ "toolError": true })),
            ),
            test_message(
                "tool-2",
                "tool",
                None,
                Some("tc-2"),
                Some(serde_json::json!({ "toolError": true })),
            ),
        ];

        let (call_name_by_id, call_signature_by_id) = build_tool_call_indices(&messages);
        let current_batch = [test_tool_call(
            "tc-3",
            "planning__clearScratchpad",
            repeated_args,
        )];

        let trigger_count = evaluate_circuit_breaker_count(
            &messages,
            &current_batch[0],
            &call_name_by_id,
            &call_signature_by_id,
        );

        assert_eq!(trigger_count, Some(3));
    }

    #[test]
    fn circuit_breaker_does_not_trigger_on_successful_signature_repetition() {
        let repeated_args = r#"{"index":3}"#;
        let messages = vec![
            test_message(
                "assistant-1",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-1",
                    "planning__checkTodo",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message("tool-1", "tool", None, Some("tc-1"), None),
            test_message(
                "assistant-2",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-2",
                    "planning__checkTodo",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message("tool-2", "tool", None, Some("tc-2"), None),
        ];

        let (call_name_by_id, call_signature_by_id) = build_tool_call_indices(&messages);
        let current_batch = [test_tool_call("tc-3", "planning__checkTodo", repeated_args)];

        let trigger_count = evaluate_circuit_breaker_count(
            &messages,
            &current_batch[0],
            &call_name_by_id,
            &call_signature_by_id,
        );

        assert_eq!(trigger_count, None);
    }

    #[test]
    fn circuit_breaker_does_not_trigger_after_five_successful_checktodo_calls() {
        let repeated_args = r#"{"index":3}"#;
        let messages = vec![
            test_message(
                "assistant-1",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-1",
                    "planning__checkTodo",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message("tool-1", "tool", None, Some("tc-1"), None),
            test_message(
                "assistant-2",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-2",
                    "planning__checkTodo",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message("tool-2", "tool", None, Some("tc-2"), None),
            test_message(
                "assistant-3",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-3",
                    "planning__checkTodo",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message("tool-3", "tool", None, Some("tc-3"), None),
            test_message(
                "assistant-4",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-4",
                    "planning__checkTodo",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message("tool-4", "tool", None, Some("tc-4"), None),
            test_message(
                "assistant-5",
                "assistant",
                Some(vec![test_tool_call(
                    "tc-5",
                    "planning__checkTodo",
                    repeated_args,
                )]),
                None,
                None,
            ),
            test_message("tool-5", "tool", None, Some("tc-5"), None),
        ];

        let (call_name_by_id, call_signature_by_id) = build_tool_call_indices(&messages);
        let current_batch = [test_tool_call("tc-6", "planning__checkTodo", repeated_args)];

        let trigger_count = evaluate_circuit_breaker_count(
            &messages,
            &current_batch[0],
            &call_name_by_id,
            &call_signature_by_id,
        );

        assert_eq!(trigger_count, None);
    }

    /// Regression test mirroring a real trace:
    /// - Tool A (healthCheck) called 3x with SUCCESS earlier in session
    /// - Tool B (readFile) called 2x with FAILURE → circuit break → resume
    /// - Tool A (healthCheck) called 3x with SUCCESS again
    ///   → 4th healthCheck attempt must NOT trigger the circuit breaker.
    ///
    /// The old code counted all repetitions regardless of success/failure,
    /// falsely triggering here. The new code breaks on the first non-error
    /// result, returning 0.
    #[test]
    fn circuit_breaker_does_not_trigger_after_success_following_different_tool_failures() {
        let health_check = "swarm__healthCheck";
        let read_file = "workspace__readFile";
        let empty_args = "{}";

        let messages = vec![
            // Earlier session: healthCheck x3 SUCCESS
            test_message(
                "assistant-a1",
                "assistant",
                Some(vec![test_tool_call("tc-a1", health_check, empty_args)]),
                None,
                None,
            ),
            test_message("tool-a1", "tool", None, Some("tc-a1"), None),
            test_message(
                "assistant-a2",
                "assistant",
                Some(vec![test_tool_call("tc-a2", health_check, empty_args)]),
                None,
                None,
            ),
            test_message("tool-a2", "tool", None, Some("tc-a2"), None),
            test_message(
                "assistant-a3",
                "assistant",
                Some(vec![test_tool_call("tc-a3", health_check, empty_args)]),
                None,
                None,
            ),
            test_message("tool-a3", "tool", None, Some("tc-a3"), None),
            // readFile x2 FAILURE
            test_message(
                "assistant-b1",
                "assistant",
                Some(vec![test_tool_call("tc-b1", read_file, empty_args)]),
                None,
                None,
            ),
            test_message(
                "tool-b1",
                "tool",
                None,
                Some("tc-b1"),
                Some(serde_json::json!({ "toolError": true })),
            ),
            test_message(
                "assistant-b2",
                "assistant",
                Some(vec![test_tool_call("tc-b2", read_file, empty_args)]),
                None,
                None,
            ),
            test_message(
                "tool-b2",
                "tool",
                None,
                Some("tc-b2"),
                Some(serde_json::json!({ "toolError": true })),
            ),
            // New session: healthCheck x3 SUCCESS again
            test_message(
                "assistant-c1",
                "assistant",
                Some(vec![test_tool_call("tc-c1", health_check, empty_args)]),
                None,
                None,
            ),
            test_message("tool-c1", "tool", None, Some("tc-c1"), None),
            test_message(
                "assistant-c2",
                "assistant",
                Some(vec![test_tool_call("tc-c2", health_check, empty_args)]),
                None,
                None,
            ),
            test_message("tool-c2", "tool", None, Some("tc-c2"), None),
            test_message(
                "assistant-c3",
                "assistant",
                Some(vec![test_tool_call("tc-c3", health_check, empty_args)]),
                None,
                None,
            ),
            test_message("tool-c3", "tool", None, Some("tc-c3"), None),
        ];

        let (call_name_by_id, call_signature_by_id) = build_tool_call_indices(&messages);
        let current_batch = [test_tool_call("tc-c4", health_check, empty_args)];

        let trigger_count = evaluate_circuit_breaker_count(
            &messages,
            &current_batch[0],
            &call_name_by_id,
            &call_signature_by_id,
        );

        assert_eq!(trigger_count, None);
    }
}
