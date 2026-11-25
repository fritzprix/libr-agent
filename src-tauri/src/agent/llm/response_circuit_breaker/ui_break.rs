use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::llm::circuit_breaker;
use crate::agent::llm::natural_recovery::LoopPreventionKind;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::models::chat::Message;

/// Whether the `ui` builtin is on for the session, so the interactive
/// `ui__circuitBreak` call can be used. Defaults to `true` when the agent
/// config cannot be resolved.
pub(super) async fn ui_alias_enabled(
    session_metadata: Option<&crate::repositories::SessionMetadata>,
    session_id: &str,
) -> bool {
    let Some(metadata) = session_metadata else {
        return true;
    };
    match crate::agent::resolve_agent_config(metadata).await {
        Ok(config) => circuit_breaker::is_builtin_alias_enabled(&config, "ui"),
        Err(error) => {
            log::warn!(
                "Failed to resolve agent config for circuit breaker in session {}: {}",
                session_id,
                error
            );
            true
        }
    }
}

/// The `ui__circuitBreak` call that stops the workflow. `reason`, when set,
/// replaces the default "identical parameters" warning text. `kind`, when set
/// (`invalidToolArgs`), travels to the Resume path (#2957) so
/// `ui__resumeCircuitBreak` can give cause-specific guidance instead of the
/// identical-parameters loop text.
pub(super) fn circuit_break_tool_call(
    tool_name: &str,
    count: usize,
    args: &str,
    reason: Option<&str>,
    kind: Option<&str>,
) -> ToolCall {
    let mut arguments = serde_json::json!({
        "toolName": tool_name,
        "repetitionCount": count,
        "args": args
    });
    if let Some(reason) = reason {
        arguments["reason"] = serde_json::json!(reason);
    }
    if let Some(kind) = kind {
        arguments["kind"] = serde_json::json!(kind);
    }
    ToolCall {
        id: uuid::Uuid::new_v4().to_string(),
        function: ToolCallFunction {
            name: "ui__circuitBreak".to_string(),
            arguments: arguments.to_string(),
        },
        r#type: "function".to_string(),
    }
}

pub(super) fn hard_break_applied_ui_circuit_break(assistant_message: &Message) -> bool {
    assistant_message
        .tool_calls
        .as_ref()
        .is_some_and(|tool_calls| {
            tool_calls
                .iter()
                .any(|tool_call| tool_call.function.name == "ui__circuitBreak")
        })
}

pub(super) fn emit_circuit_breaker_triggered(
    session_id: &str,
    tool_name: &str,
    count: usize,
    action: &str,
) {
    let safe_tool = circuit_breaker::sanitize_circuit_breaker_log_tool_name(tool_name);
    let Some(app_handle) = crate::state::get_app_handle() else {
        return;
    };
    let _ = TauriEventDispatcher::from_handle(app_handle).emit_agent_event(
        AgentEvent::CircuitBreakerTriggered {
            session_id: session_id.to_string(),
            tool_name: safe_tool,
            count,
            action: action.to_string(),
        },
    );
}

pub(super) fn circuit_breaker_action_label(kind: &LoopPreventionKind) -> &'static str {
    match kind {
        LoopPreventionKind::RepeatedErrorOutcome | LoopPreventionKind::RepeatedSuccessOutcome => {
            "softRecovery"
        }
        LoopPreventionKind::RepeatedErrorEscalate => "errorEscalate",
        LoopPreventionKind::RepeatedBatchSequence => "repeatedBatch",
        LoopPreventionKind::RepeatedBatchSequenceEscalate => "repeatedBatchEscalate",
        LoopPreventionKind::DuplicateInBatch => "duplicateInBatch",
        LoopPreventionKind::WaitGuidance => "waitGuidance",
    }
}
