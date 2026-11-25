use super::ui_break::{circuit_break_tool_call, emit_circuit_breaker_triggered, ui_alias_enabled};
use crate::agent::llm::circuit_breaker;
use crate::agent::state::AgentSession;
use crate::mcp::builtin::ui::CIRCUIT_BREAK_KIND_INVALID_TOOL_ARGS;
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Clean resamples allowed after a reply whose tool-call arguments are not a
/// valid JSON object. When the original reply and all resamples fail, the
/// circuit breaker stops the turn.
pub const INVALID_TOOL_ARGS_MAX_RESAMPLES: u32 = 3;

/// Result of [`apply_invalid_tool_args_policy`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidToolArgsOutcome {
    /// No tool call has invalid arguments. Continue as usual.
    Valid,
    /// Discard the reply (do not cache, persist, or execute it) and request a
    /// new completion for the same history.
    Resample {
        tool_name: String,
        attempt: u32,
        max_resamples: u32,
    },
    /// The resample budget is spent. The reply now holds a circuit break (or a
    /// text-only stop) instead of the invalid tool calls.
    CircuitBreak { tool_name: String, attempts: u32 },
}

/// Decide what to do with a reply whose tool-call arguments are cut off (for
/// example, at the output-token limit) or are not a JSON object.
///
/// Such a reply must not go into history: the arguments are not executable,
/// and replaying them makes some OpenAI-compatible servers fail with HTTP 500.
/// The reply is resampled up to [`INVALID_TOOL_ARGS_MAX_RESAMPLES`] times.
/// After that, the invalid calls are replaced with `ui__circuitBreak` (valid
/// JSON arguments only), so the workflow stops with a clear message.
pub(crate) async fn apply_invalid_tool_args_policy(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) -> InvalidToolArgsOutcome {
    let invalid = assistant_message.tool_calls.as_deref().and_then(|calls| {
        calls.iter().find_map(|call| {
            crate::agent::llm::tool_args_validation::inspect_tool_call_arguments(
                &call.function.arguments,
            )
            .err()
            .map(|(kind, _)| {
                (
                    call.function.name.clone(),
                    call.function.arguments.len(),
                    kind,
                )
            })
        })
    });

    let (session_metadata, retry_count) = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return InvalidToolArgsOutcome::Valid;
        };
        (
            session.metadata.clone(),
            session.invalid_tool_args_retry_count.clone(),
        )
    };

    let Some((tool_name, raw_len, kind)) = invalid else {
        *retry_count.write().await = 0;
        return InvalidToolArgsOutcome::Valid;
    };
    let safe_tool = circuit_breaker::sanitize_circuit_breaker_log_tool_name(&tool_name);

    {
        let mut count = retry_count.write().await;
        if *count < INVALID_TOOL_ARGS_MAX_RESAMPLES {
            *count += 1;
            log::warn!(
                "Invalid tool-call arguments for session {} tool {} ({}, {} bytes). Discarding the reply and resampling ({}/{}).",
                session_id,
                safe_tool,
                kind.as_error_kind(),
                raw_len,
                *count,
                INVALID_TOOL_ARGS_MAX_RESAMPLES
            );
            return InvalidToolArgsOutcome::Resample {
                tool_name,
                attempt: *count,
                max_resamples: INVALID_TOOL_ARGS_MAX_RESAMPLES,
            };
        }
        *count = 0;
    }

    let attempts = INVALID_TOOL_ARGS_MAX_RESAMPLES + 1;
    log::warn!(
        "Circuit breaker triggered for session {} tool {}: invalid tool-call arguments in {} replies in a row ({})",
        session_id,
        safe_tool,
        attempts,
        kind.as_error_kind()
    );
    emit_circuit_breaker_triggered(
        session_id,
        &tool_name,
        attempts as usize,
        CIRCUIT_BREAK_KIND_INVALID_TOOL_ARGS,
    );

    let args_marker = if kind.is_truncated() {
        crate::agent::llm::tool_args_validation::redact_tool_call_arguments(raw_len)
    } else {
        format!("<redacted: {} bytes, malformed>", raw_len)
    };
    let reason = format!(
        "The model sent invalid arguments for the tool \"{}\" in {} replies in a row ({}). The tool was not run. The workflow stopped to prevent a provider error. Click Resume to continue, or change your request.",
        safe_tool,
        attempts,
        kind.as_error_kind()
    );

    if ui_alias_enabled(Some(&session_metadata), session_id).await {
        assistant_message.tool_calls = Some(vec![circuit_break_tool_call(
            &tool_name,
            attempts as usize,
            &args_marker,
            Some(&reason),
            // #2957: the Resume text must match the cause.
            Some(CIRCUIT_BREAK_KIND_INVALID_TOOL_ARGS),
        )]);
    } else {
        assistant_message.tool_calls = None;
        assistant_message.content = vec![crate::mcp::types::MCPContent::Text {
            text: format!("⚠️ Circuit breaker triggered: {reason}"),
        }];
    }

    InvalidToolArgsOutcome::CircuitBreak {
        tool_name,
        attempts,
    }
}

pub async fn apply_invalid_tool_args_policy_for_testing(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) -> InvalidToolArgsOutcome {
    apply_invalid_tool_args_policy(active_sessions, session_id, assistant_message).await
}
