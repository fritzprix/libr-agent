//! Circuit-breaker resume support for the E2E control plane (#2901).
//!
//! The interactive circuit breaker (`ui__circuitBreak`) stops the workflow and
//! waits for the user to click Resume, which injects an assistant message whose
//! only tool call is `ui__resumeCircuitBreak`. The control plane observes that
//! waiting state and injects the same message, so an unattended harness can
//! drive the Resume path without a GUI.
//!
//! This is deliberately NOT `POST /api/sessions/:id/resume`: that route reloads
//! a paused or crashed session (#2898) and never touches the circuit-breaker
//! dialog.

use serde::Serialize;

use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::models::chat::Message;

/// How many recent messages the pending-circuit-break scan inspects.
const SCAN_WINDOW: usize = 64;

/// A circuit breaker that fired and is still waiting for Resume.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PendingCircuitBreak {
    pub tool_name: String,
    pub repetition_count: u64,
    pub tool_call_id: String,
}

/// Strip the MCP server prefix from a tool name (`ui__circuitBreak` → `circuitBreak`).
fn tool_base_name(name: &str) -> &str {
    name.rsplit("__").next().unwrap_or(name)
}

/// The circuit break currently waiting for a Resume, if any.
///
/// Scans the recent messages backwards: an assistant tool call to
/// `resumeCircuitBreak` means the break was already resumed; a new user
/// message after the break means the session moved on without Resume; a
/// call to `circuitBreak` reached first means it is still pending.
pub fn pending_circuit_break(messages: &[Message]) -> Option<PendingCircuitBreak> {
    for message in messages.iter().rev().take(SCAN_WINDOW) {
        if message.role == "user" {
            // The user sent a new message after the break, so the workflow
            // moved on: the break is no longer waiting for Resume and the
            // harness must not inject one into a later turn.
            return None;
        }
        let Some(tool_calls) = message.tool_calls.as_ref() else {
            continue;
        };
        for tool_call in tool_calls {
            match tool_base_name(&tool_call.function.name) {
                "resumeCircuitBreak" => return None,
                "circuitBreak" => {
                    let args: serde_json::Value =
                        serde_json::from_str(&tool_call.function.arguments).unwrap_or_default();
                    return Some(PendingCircuitBreak {
                        tool_name: args
                            .get("toolName")
                            .and_then(|value| value.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        repetition_count: args
                            .get("repetitionCount")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(0),
                        tool_call_id: tool_call.id.clone(),
                    });
                }
                _ => {}
            }
        }
    }
    None
}

/// Build the assistant message that the Resume button injects, so the control
/// plane drives the exact same path as the GUI.
pub fn resume_circuit_break_message(
    session_id: &str,
    tool_name: &str,
    repetition_count: u64,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let arguments = serde_json::json!({
        "toolName": tool_name,
        "repetitionCount": repetition_count,
    })
    .to_string();
    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: vec![],
        tool_calls: Some(vec![ToolCall {
            id: format!("e2e-resume-{}", uuid::Uuid::new_v4()),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "ui__resumeCircuitBreak".to_string(),
                arguments,
            },
        }]),
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at: now,
        updated_at: now,
        source: None,
        error: None,
        metadata: None,
    }
}
