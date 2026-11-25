use super::ToolLoopResampleDecision;
use crate::agent::llm::circuit_breaker;
use crate::agent::llm::natural_recovery::{LoopPreventionKind, LoopPreventionShortCircuit};
use crate::agent::state::AgentSession;
use crate::agent::types::ToolCall;
use crate::repositories::settings_repository::SettingsRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) fn resample_budget_key(
    short_circuit: &LoopPreventionShortCircuit,
    current_tool_calls: &[ToolCall],
) -> String {
    match short_circuit.kind {
        LoopPreventionKind::RepeatedBatchSequence
        | LoopPreventionKind::RepeatedBatchSequenceEscalate => format!(
            "batch:{}",
            circuit_breaker::batch_fingerprint(current_tool_calls)
        ),
        _ => format!(
            "{}:{}",
            short_circuit.tool_name,
            circuit_breaker::normalize_tool_arguments(&short_circuit.args)
        ),
    }
}

pub(super) async fn session_resample_attempts(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    budget_key: &str,
) -> usize {
    let active = active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return 0;
    };
    let attempts = session.tool_loop_resample_attempts.read().await;
    *attempts.get(budget_key).unwrap_or(&0)
}

pub(crate) async fn increment_tool_loop_resample_attempt(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    budget_key: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        let mut attempts = session.tool_loop_resample_attempts.write().await;
        *attempts.entry(budget_key.to_string()).or_insert(0) += 1;
    }
}

pub(super) async fn load_tool_loop_resample_settings() -> (bool, usize) {
    // Default policy: clean resample (no intrusive guidance injection).
    // Legacy guidance is opt-in via `toolLoopRecoveryPolicy: "legacyGuidance"`.
    let default_resample_enabled = true;
    let default_max_retries = 2usize;

    let Some(settings_repo) = crate::state::try_get_settings_repository() else {
        return (default_resample_enabled, default_max_retries);
    };

    match settings_repo.get("experimentalSettings").await {
        Ok(Some(model)) => {
            let parsed = match serde_json::from_str::<serde_json::Value>(&model.value) {
                Ok(value) => value,
                Err(error) => {
                    log::warn!(
                        "Malformed experimentalSettings JSON; using tool-loop resample defaults: {}",
                        error
                    );
                    serde_json::Value::Null
                }
            };

            let resample_enabled = match parsed
                .get("toolLoopRecoveryPolicy")
                .and_then(|v| v.as_str())
            {
                Some("legacyGuidance") => false,
                Some("resampleThenBreak") => true,
                _ => {
                    // Read-compat: deprecated boolean (inverted: true = legacy guidance).
                    parsed
                        .get("toolLoopLegacyGuidanceEnabled")
                        .and_then(|v| v.as_bool())
                        .map(|legacy| !legacy)
                        .unwrap_or(default_resample_enabled)
                }
            };

            // Allow 0 for "no resamples" (immediate circuit breaker at attempt_index=0).
            let max_retries = parsed
                .get("toolLoopMaxResampleRetries")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(default_max_retries)
                .clamp(0, 20);

            (resample_enabled, max_retries)
        }
        _ => (default_resample_enabled, default_max_retries),
    }
}

pub(super) async fn select_resample_decision(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    loop_prevention_short_circuits: &HashMap<String, LoopPreventionShortCircuit>,
    tool_calls: &[ToolCall],
    tool_loop_max_resample_retries: usize,
) -> Option<ToolLoopResampleDecision> {
    let repeated_candidates = loop_prevention_short_circuits.values().filter(|sc| {
        matches!(
            sc.kind,
            LoopPreventionKind::RepeatedErrorOutcome
                | LoopPreventionKind::RepeatedErrorEscalate
                | LoopPreventionKind::RepeatedSuccessOutcome
                | LoopPreventionKind::RepeatedBatchSequence
                | LoopPreventionKind::RepeatedBatchSequenceEscalate
        )
    });

    let best = repeated_candidates.max_by_key(|sc| sc.count)?;
    let budget_key = resample_budget_key(best, tool_calls);
    let session_attempts =
        session_resample_attempts(active_sessions, session_id, &budget_key).await;
    if session_attempts < tool_loop_max_resample_retries {
        Some(ToolLoopResampleDecision {
            tool_name: best.tool_name.clone(),
            kind: best.kind.clone(),
            attempt_index: session_attempts,
            count: best.count,
            max_retries: tool_loop_max_resample_retries,
            budget_key,
            args: best.args.clone(),
        })
    } else {
        None
    }
}
