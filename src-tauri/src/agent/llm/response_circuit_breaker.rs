use crate::agent::llm::circuit_breaker;
use crate::agent::state::AgentSession;
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(crate) async fn preprocess_assistant_tool_calls(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) {
    let mut forced_circuit_break_message = None;

    if let Some(tool_calls) = &mut assistant_message.tool_calls {
        let mut break_index = None;
        let mut break_action = None;
        let mut ui_alias_enabled = true;
        let loop_threshold = circuit_breaker::load_loop_prevention_threshold().await;

        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            ui_alias_enabled = circuit_breaker::is_builtin_alias_enabled(
                session.metadata.agent_config.as_deref(),
                "ui",
            );
            let messages = session.messages.read().await;
            let call_signature_by_id = circuit_breaker::build_tool_call_indices(&messages);

            for (index, tool_call) in tool_calls.iter().enumerate() {
                if let Some(action) = circuit_breaker::evaluate_circuit_breaker_action(
                    &messages,
                    tool_call,
                    &call_signature_by_id,
                    loop_threshold,
                ) {
                    break_index = Some(index);
                    break_action = Some(action);
                    break;
                }
            }
        }
        drop(sessions);

        if let Some(index) = break_index {
            if let Some(action) = break_action {
                match action {
                    circuit_breaker::CircuitBreakerAction::HardBreak {
                        count,
                        tool_name,
                        args,
                    } => {
                        log::warn!(
                            "Circuit breaker triggered for session {} tool {} (count {})",
                            session_id,
                            tool_name,
                            count
                        );

                        if ui_alias_enabled {
                            let circuit_break_call = ToolCall {
                                id: uuid::Uuid::new_v4().to_string(),
                                function: ToolCallFunction {
                                    name: "ui__circuitBreak".to_string(),
                                    arguments: serde_json::json!({
                                        "toolName": tool_name,
                                        "repetitionCount": count,
                                        "args": args
                                    })
                                    .to_string(),
                                },
                                r#type: "function".to_string(),
                            };

                            tool_calls[index] = circuit_break_call;
                            tool_calls.truncate(index + 1);
                        } else {
                            log::warn!(
                                "UI alias disabled for session {}. Using text-only circuit break fallback.",
                                session_id
                            );

                            forced_circuit_break_message =
                                Some(crate::mcp::types::MCPContent::Text {
                                    text: format!(
                                        "⚠️ Circuit breaker triggered: detected runaway loop for tool '{}' (count {}).\n\nThe 'ui' builtin server is disabled for this session, so interactive circuit-break UI was skipped. Workflow was force-stopped to prevent further runaway calls.",
                                        tool_name, count
                                    ),
                                    is_error: None,
                                });
                        }
                    }
                    circuit_breaker::CircuitBreakerAction::NaturalRecoveryError {
                        count,
                        tool_name,
                        ..
                    } => {
                        log::warn!(
                            "Natural recovery (Error track) triggered for session {} tool {} (count {})",
                            session_id, tool_name, count
                        );
                        let entropy = uuid::Uuid::new_v4().to_string();
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_nanos() as usize;

                        let error_templates = [
                            "Wait, my action '{TOOL_NAME}' keeps failing and I am stuck in a loop. I must reflect on my previous state and consider a completely different alternative approach instead of repeating the identical action.",
                            "The tool '{TOOL_NAME}' has resulted in an error repeatedly. Let me stop using it and think about another way to achieve the goal.",
                            "Attempting '{TOOL_NAME}' with the same arguments is clearly not working. I should review the error messages carefully and change my strategy.",
                            "I'm caught in an error loop with '{TOOL_NAME}'. Let's halt this action. What am I missing in the configuration or arguments?",
                            "Calling '{TOOL_NAME}' again won't fix the issue. I need to formulate a new plan and avoid the path that leads to this failure.",
                            "I keep hitting the same wall with '{TOOL_NAME}'. Let me step back, analyze the root cause of this error, and try a different tool.",
                            "This repeated failure on '{TOOL_NAME}' indicates my approach is flawed. I must deviate from this pattern immediately and re-evaluate.",
                            "I must break this cycle. '{TOOL_NAME}' is consistently failing. I will stop executing it and instead focus on debugging the core problem.",
                            "There's no point in trying '{TOOL_NAME}' one more time here. I need to take a fundamentally different approach to this task.",
                            "I am stuck. The same error keeps popping up for '{TOOL_NAME}'. Let me pause, clear my assumptions, and look for an alternative method."
                        ];

                        let template = error_templates[nanos % error_templates.len()];
                        let recovery_thought = format!(
                            "{} [Entropy ID: {}]",
                            template.replace("{TOOL_NAME}", &tool_name),
                            entropy
                        );

                        let think_call = ToolCall {
                            id: uuid::Uuid::new_v4().to_string(),
                            function: ToolCallFunction {
                                name: "scratchpad__think".to_string(),
                                arguments: serde_json::json!({
                                    "thought": recovery_thought
                                })
                                .to_string(),
                            },
                            r#type: "function".to_string(),
                        };
                        tool_calls[index] = think_call;
                        tool_calls.truncate(index + 1);
                    }
                    circuit_breaker::CircuitBreakerAction::NaturalRecoverySuccess {
                        count,
                        tool_name,
                        ..
                    } => {
                        log::warn!(
                            "Natural recovery (Success track) triggered for session {} tool {} (count {})",
                            session_id, tool_name, count
                        );
                        let entropy = uuid::Uuid::new_v4().to_string();
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_nanos() as usize;

                        let success_templates = [
                            "I have repeatedly called '{TOOL_NAME}' successfully with identical parameters but I am not making progress. What was I originally scheduled to do? I need to focus on the next step immediately.",
                            "The repeated success of '{TOOL_NAME}' means the state has changed as intended, but I'm inexplicably repeating it. I must move forward to the next logical task.",
                            "Executing '{TOOL_NAME}' over and over with the same inputs is redundant. I have already achieved the result of this step. Time to proceed.",
                            "I'm looping on '{TOOL_NAME}' even though it's succeeding. I must break out of this repetition and execute the next action in my plan.",
                            "Why am I doing this? '{TOOL_NAME}' was already successful. Let me read my original plan and advance to the next unmet objective.",
                            "I need to advance. The repeated execution of '{TOOL_NAME}' is a loop. Let me stop this and focus on what remains to be done.",
                            "This is a redundant success loop. '{TOOL_NAME}' worked, so I should stop calling it and move on to the next phase of the workflow.",
                            "I've verified that '{TOOL_NAME}' succeeds. There's no need to rerun it. I will check my task list and transition to the subsequent step.",
                            "I am stuck in a pattern of repeating '{TOOL_NAME}'. I need to break this habit immediately and progress with the remainder of my objective.",
                            "Success on '{TOOL_NAME}' is verified. Repeating it adds no value. Let me pivot to the next action required to complete my goal."
                        ];

                        let template = success_templates[nanos % success_templates.len()];
                        let recovery_thought = format!(
                            "{} [Entropy ID: {}]",
                            template.replace("{TOOL_NAME}", &tool_name),
                            entropy
                        );

                        let think_call = ToolCall {
                            id: uuid::Uuid::new_v4().to_string(),
                            function: ToolCallFunction {
                                name: "scratchpad__think".to_string(),
                                arguments: serde_json::json!({
                                    "thought": recovery_thought
                                })
                                .to_string(),
                            },
                            r#type: "function".to_string(),
                        };
                        tool_calls[index] = think_call;
                        tool_calls.truncate(index + 1);
                    }
                }
            }
        }
    }

    if let Some(circuit_break_message) = forced_circuit_break_message {
        assistant_message.tool_calls = None;
        assistant_message.content = vec![circuit_break_message];
    }
}
