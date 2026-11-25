//! Circuit-breaker preprocessing for assistant tool-call batches.
//!
//! Evaluates loop-prevention and experimental resample-then-break policy before
//! assistant messages are cached, persisted, or executed.

mod invalid_args;
mod preprocess;
mod resample;
mod token_fence;
mod ui_break;

use crate::agent::llm::circuit_breaker;
use crate::agent::llm::natural_recovery::{LoopPreventionKind, LoopPreventionShortCircuit};
use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(crate) use invalid_args::apply_invalid_tool_args_policy;
pub use invalid_args::{
    apply_invalid_tool_args_policy_for_testing, InvalidToolArgsOutcome,
    INVALID_TOOL_ARGS_MAX_RESAMPLES,
};
pub(crate) use resample::increment_tool_loop_resample_attempt;

/// Outcome of `preprocess_assistant_tool_calls` after loop detection runs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CircuitBreakerPreprocessResult {
    pub loop_prevention_short_circuits: HashMap<String, LoopPreventionShortCircuit>,
    /// Experimental resample-then-break decision for repeated tool-loop signatures.
    ///
    /// When set, the response layer will trigger a bounded "clean resample"
    /// completion retry and skip tool execution (so no intrusive loop-prevention
    /// guidance gets injected as tool-error text).
    pub tool_loop_resample: Option<ToolLoopResampleDecision>,
    pub forced_stop: Option<ForcedCircuitBreakStop>,
}

/// Hard-stop path when the identical-(call, outcome) streak exceeds
/// `loopPreventionThreshold + hardBreakOffset`.
#[derive(Debug, PartialEq, Eq)]
pub enum ForcedCircuitBreakStop {
    /// `ui__circuitBreak` was injected into `assistant_message.tool_calls`.
    InteractiveCircuitBreak,
    /// `assistant_message` was converted to text-only forced stop content.
    TextOnly,
}

/// Plan for a bounded resample retry instead of injecting loop-prevention
/// guidance as tool-error text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolLoopResampleDecision {
    pub tool_name: String,
    pub kind: LoopPreventionKind,
    /// Session-scoped resample attempt index for this loop fingerprint.
    pub attempt_index: usize,
    pub count: usize,
    pub max_retries: usize,
    /// Key into `AgentSession.tool_loop_resample_attempts`.
    pub budget_key: String,
    pub args: String,
}

pub(crate) async fn preprocess_assistant_tool_calls(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) -> CircuitBreakerPreprocessResult {
    let mut forced_circuit_break_message = None;
    let mut loop_prevention_short_circuits = HashMap::new();
    let mut tool_loop_resample: Option<ToolLoopResampleDecision> = None;

    if let Some(tool_calls) = assistant_message.tool_calls.clone() {
        let (loop_threshold, loop_break_offset, wait_settings) =
            circuit_breaker::load_loop_prevention_settings().await;
        let (tool_loop_resample_enabled, tool_loop_max_resample_retries) =
            resample::load_tool_loop_resample_settings().await;

        let (session_metadata, hard_break, short_circuits) =
            preprocess::collect_loop_prevention_signals(
                active_sessions,
                session_id,
                &tool_calls,
                loop_threshold,
                loop_break_offset,
                &wait_settings,
                tool_loop_resample_enabled,
                tool_loop_max_resample_retries,
            )
            .await;
        loop_prevention_short_circuits = short_circuits;

        if let Some(hard_break_info) = hard_break {
            loop_prevention_short_circuits.clear();
            forced_circuit_break_message = preprocess::apply_hard_break(
                session_id,
                session_metadata.as_ref(),
                assistant_message,
                hard_break_info,
            )
            .await;
        } else if !loop_prevention_short_circuits.is_empty() {
            if tool_loop_resample_enabled {
                tool_loop_resample = resample::select_resample_decision(
                    active_sessions,
                    session_id,
                    &loop_prevention_short_circuits,
                    &tool_calls,
                    tool_loop_max_resample_retries,
                )
                .await;
            }
            preprocess::log_and_emit_short_circuits(
                session_id,
                &loop_prevention_short_circuits,
                tool_loop_resample.is_none(),
            );
        }
    }

    if let Some(circuit_break_message) = forced_circuit_break_message {
        assistant_message.tool_calls = None;
        assistant_message.content = vec![circuit_break_message];
        return CircuitBreakerPreprocessResult {
            loop_prevention_short_circuits: HashMap::new(),
            tool_loop_resample: None,
            forced_stop: Some(ForcedCircuitBreakStop::TextOnly),
        };
    }

    if tool_loop_resample.is_none() {
        token_fence::apply_tool_loop_token_fence(active_sessions, session_id, assistant_message)
            .await;
    }

    let forced_stop = if ui_break::hard_break_applied_ui_circuit_break(assistant_message) {
        Some(ForcedCircuitBreakStop::InteractiveCircuitBreak)
    } else {
        None
    };

    CircuitBreakerPreprocessResult {
        loop_prevention_short_circuits,
        tool_loop_resample,
        forced_stop,
    }
}

pub async fn preprocess_assistant_tool_calls_for_testing(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) -> CircuitBreakerPreprocessResult {
    preprocess_assistant_tool_calls(active_sessions, session_id, assistant_message).await
}
