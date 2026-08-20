use crate::agent::llm::completion::request_llm_completion_with_recovery;
use crate::agent::llm::response::finalize_workflow_error_with_dispatcher;
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompletionCancelRequest, StreamingIssueKind,
    StreamingIssueReport,
};
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::emit_completion_cancel;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub const REPEATED_THINKING_MAX_RETRIES: u32 = 2;
pub const REPEATED_TEXT_LOOP_MAX_RETRIES: u32 = 2;
pub const BAD_TOOL_ARGS_MAX_RETRIES: u32 = 2;
/// Client-side reasoning-budget abort: one clean resample, no prompt nudge.
pub const REASONING_BUDGET_MAX_RETRIES: u32 = 1;
/// Max FallThrough incidents per workflow before hard-stopping. Bounds
/// truncated-args loops where raw args (and thus loop-prevention signatures)
/// change every attempt.
pub const BAD_TOOL_ARGS_MAX_INCIDENTS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingIssueAction {
    Ignore,
    CancelAndRetry { next_retry_count: u32 },
    CancelAndFail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingIssueOutcome {
    Ignored,
    Retried {
        retry_count: u32,
    },
    /// Budget exhausted for a recoverable bad response; caller should continue
    /// with the original completion (e.g. guided tool-error close).
    FallThrough,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonProductiveCompletionAction {
    Retry { next_retry_count: u32 },
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonProductiveCompletionReason {
    RepeatedThinkingLoop {
        response_message_id: String,
        observed_tail_chars: usize,
        pattern_length: usize,
        repetition_count: usize,
    },
    RepeatedTextLoop {
        response_message_id: String,
        observed_tail_chars: usize,
        pattern_length: usize,
        repetition_count: usize,
    },
    ThinkingOnlyCompletion {
        assistant_message_id: String,
    },
    MalformedToolCallArguments {
        assistant_message_id: String,
        tool_names: Vec<String>,
        /// Primary failure kind from the first invalid tool call (for logging).
        parse_kind: String,
    },
    ReasoningBudgetExceeded {
        response_message_id: String,
        observed_tail_chars: usize,
        /// floor(max_tokens * 0.9) from the frontend report.
        reasoning_budget_tokens: usize,
        estimated_thinking_tokens: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamingRecoveryCounter {
    Thinking,
    Text,
    BadToolArgs,
    ReasoningBudget,
}

impl StreamingRecoveryCounter {
    fn max_retries(self) -> u32 {
        match self {
            Self::Thinking => REPEATED_THINKING_MAX_RETRIES,
            Self::Text => REPEATED_TEXT_LOOP_MAX_RETRIES,
            Self::BadToolArgs => BAD_TOOL_ARGS_MAX_RETRIES,
            Self::ReasoningBudget => REASONING_BUDGET_MAX_RETRIES,
        }
    }

    async fn read_count(self, session: &AgentSession) -> u32 {
        match self {
            Self::Thinking => *session.repeated_thinking_retry_count.read().await,
            Self::Text => *session.repeated_text_loop_retry_count.read().await,
            Self::BadToolArgs => *session.bad_tool_args_retry_count.read().await,
            Self::ReasoningBudget => *session.reasoning_budget_retry_count.read().await,
        }
    }

    async fn write_count(self, session: &AgentSession, value: u32) {
        match self {
            Self::Thinking => {
                *session.repeated_thinking_retry_count.write().await = value;
            }
            Self::Text => {
                *session.repeated_text_loop_retry_count.write().await = value;
            }
            Self::BadToolArgs => {
                *session.bad_tool_args_retry_count.write().await = value;
            }
            Self::ReasoningBudget => {
                *session.reasoning_budget_retry_count.write().await = value;
            }
        }
    }
}

struct RecoveryContext<'a> {
    session_repo: &'a Arc<dyn SessionRepository>,
    active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &'a Arc<MCPServiceProxyManager>,
    app_handle: &'a AppHandle,
}

pub fn evaluate_non_productive_completion_action(
    retry_count: u32,
    max_retries: u32,
) -> NonProductiveCompletionAction {
    if retry_count < max_retries {
        return NonProductiveCompletionAction::Retry {
            next_retry_count: retry_count + 1,
        };
    }

    NonProductiveCompletionAction::Fail
}

pub fn evaluate_streaming_issue_action(
    expected_response_id: Option<&str>,
    reported_response_id: &str,
    retry_count: u32,
    max_retries: u32,
) -> StreamingIssueAction {
    if expected_response_id != Some(reported_response_id) {
        return StreamingIssueAction::Ignore;
    }

    match evaluate_non_productive_completion_action(retry_count, max_retries) {
        NonProductiveCompletionAction::Retry { next_retry_count } => {
            StreamingIssueAction::CancelAndRetry { next_retry_count }
        }
        NonProductiveCompletionAction::Fail => StreamingIssueAction::CancelAndFail,
    }
}

async fn handle_non_productive_completion(
    context: RecoveryContext<'_>,
    session_id: String,
    session_name: String,
    counter: StreamingRecoveryCounter,
    action: NonProductiveCompletionAction,
    reason: NonProductiveCompletionReason,
) -> Result<StreamingIssueOutcome, String> {
    let max_retries = counter.max_retries();

    match action {
        NonProductiveCompletionAction::Retry { next_retry_count } => {
            {
                let active = context.active_sessions.read().await;
                if let Some(session) = active.get(&session_id) {
                    counter.write_count(session, next_retry_count).await;
                }
            }

            match &reason {
                NonProductiveCompletionReason::RepeatedThinkingLoop {
                    response_message_id,
                    observed_tail_chars,
                    pattern_length,
                    repetition_count,
                } => {
                    log::warn!(
                        "Repeated thinking loop detected for session {} response {} (tail_chars={}, pattern_length={}, repetition_count={}). Retrying LLM turn ({}/{}).",
                        session_id,
                        response_message_id,
                        observed_tail_chars,
                        pattern_length,
                        repetition_count,
                        next_retry_count,
                        max_retries
                    );
                }
                NonProductiveCompletionReason::RepeatedTextLoop {
                    response_message_id,
                    observed_tail_chars,
                    pattern_length,
                    repetition_count,
                } => {
                    log::warn!(
                        "Repeated text loop detected for session {} response {} (tail_chars={}, pattern_length={}, repetition_count={}). Retrying LLM turn ({}/{}).",
                        session_id,
                        response_message_id,
                        observed_tail_chars,
                        pattern_length,
                        repetition_count,
                        next_retry_count,
                        max_retries
                    );
                }
                NonProductiveCompletionReason::ThinkingOnlyCompletion {
                    assistant_message_id,
                } => {
                    log::warn!(
                        "Thinking-only completion detected for session {} message {}. Retrying LLM turn ({}/{}).",
                        session_id,
                        assistant_message_id,
                        next_retry_count,
                        max_retries
                    );
                }
                NonProductiveCompletionReason::MalformedToolCallArguments {
                    assistant_message_id,
                    tool_names,
                    parse_kind,
                } => {
                    log::warn!(
                        "Malformed/truncated tool-call arguments detected for session {} message {} (tools={:?}, kind={}). Retrying LLM turn ({}/{}).",
                        session_id,
                        assistant_message_id,
                        tool_names,
                        parse_kind,
                        next_retry_count,
                        max_retries
                    );
                }
                NonProductiveCompletionReason::ReasoningBudgetExceeded {
                    response_message_id,
                    observed_tail_chars,
                    reasoning_budget_tokens,
                    estimated_thinking_tokens,
                } => {
                    log::warn!(
                        "Reasoning budget exceeded for session {} response {} (thinking_chars={}, reasoning_budget_tokens={}, estimated_thinking_tokens={}). Retrying LLM turn ({}/{}).",
                        session_id,
                        response_message_id,
                        observed_tail_chars,
                        reasoning_budget_tokens,
                        estimated_thinking_tokens,
                        next_retry_count,
                        max_retries
                    );
                }
            }

            let recovery_result = request_llm_completion_with_recovery(
                context.session_repo,
                context.active_sessions,
                context.proxy_manager,
                context.app_handle,
                session_id.clone(),
            )
            .await;

            if let Err(error) = recovery_result {
                // Malformed-args retries must still close tool calls if the
                // replacement completion cannot start — fall through instead of
                // aborting handle_llm_response with `?`.
                if matches!(
                    reason,
                    NonProductiveCompletionReason::MalformedToolCallArguments { .. }
                ) {
                    log::warn!(
                        "Bad-tool-args retry failed to start replacement completion for session {}: {}. Falling through to guided tool-error close.",
                        session_id,
                        error
                    );
                    return fallthrough_or_hard_fail_malformed_tool_args(
                        &context,
                        &session_id,
                        &session_name,
                        &reason,
                        max_retries,
                    )
                    .await;
                }
                return Err(error);
            }

            Ok(StreamingIssueOutcome::Retried {
                retry_count: next_retry_count,
            })
        }
        NonProductiveCompletionAction::Fail => {
            if matches!(
                reason,
                NonProductiveCompletionReason::MalformedToolCallArguments { .. }
            ) {
                return fallthrough_or_hard_fail_malformed_tool_args(
                    &context,
                    &session_id,
                    &session_name,
                    &reason,
                    max_retries,
                )
                .await;
            }

            {
                let active = context.active_sessions.read().await;
                if let Some(session) = active.get(&session_id) {
                    counter.write_count(session, 0).await;
                }
            }

            let (message, code, details) = match reason {
                NonProductiveCompletionReason::RepeatedThinkingLoop {
                    response_message_id,
                    observed_tail_chars,
                    pattern_length,
                    repetition_count,
                } => (
                    format!(
                        "The model got stuck repeating thinking content in session '{}' and exceeded the automatic recovery limit. Workflow stopped to prevent an infinite loop.",
                        session_name
                    ),
                    "REPEATED_THINKING_LOOP",
                    serde_json::json!({
                        "responseMessageId": response_message_id,
                        "observedTailChars": observed_tail_chars,
                        "patternLength": pattern_length,
                        "repetitionCount": repetition_count,
                        "maxRetries": max_retries,
                    }),
                ),
                NonProductiveCompletionReason::RepeatedTextLoop {
                    response_message_id,
                    observed_tail_chars,
                    pattern_length,
                    repetition_count,
                } => (
                    format!(
                        "The model got stuck repeating text content in session '{}' and exceeded the automatic recovery limit. Workflow stopped to prevent an infinite loop.",
                        session_name
                    ),
                    "REPEATED_TEXT_LOOP",
                    serde_json::json!({
                        "responseMessageId": response_message_id,
                        "observedTailChars": observed_tail_chars,
                        "patternLength": pattern_length,
                        "repetitionCount": repetition_count,
                        "maxRetries": max_retries,
                    }),
                ),
                NonProductiveCompletionReason::ThinkingOnlyCompletion {
                    assistant_message_id,
                } => (
                    format!(
                        "The model returned only thinking content in session '{}' and exceeded the automatic recovery limit. Workflow stopped because no usable assistant output was produced.",
                        session_name
                    ),
                    "THINKING_ONLY_COMPLETION",
                    serde_json::json!({
                        "assistantMessageId": assistant_message_id,
                        "maxRetries": max_retries,
                    }),
                ),
                NonProductiveCompletionReason::MalformedToolCallArguments { .. } => {
                    unreachable!("MalformedToolCallArguments Fail handled above")
                }
                NonProductiveCompletionReason::ReasoningBudgetExceeded {
                    response_message_id,
                    observed_tail_chars,
                    reasoning_budget_tokens,
                    estimated_thinking_tokens,
                } => (
                    format!(
                        "The model exceeded the reasoning budget in session '{}' and already used the automatic recovery retry. Workflow stopped to prevent unbounded thinking.",
                        session_name
                    ),
                    "REASONING_BUDGET_EXCEEDED",
                    serde_json::json!({
                        "responseMessageId": response_message_id,
                        "observedTailChars": observed_tail_chars,
                        "reasoningBudgetTokens": reasoning_budget_tokens,
                        "estimatedThinkingTokens": estimated_thinking_tokens,
                        "maxRetries": max_retries,
                    }),
                ),
            };

            finalize_workflow_error_with_dispatcher(
                context.session_repo,
                context.active_sessions,
                context.app_handle,
                session_id,
                AgentRuntimeError::new(AgentRuntimeErrorType::AiServiceError, message)
                    .with_code(code)
                    .with_original_error(details),
            )
            .await?;

            Ok(StreamingIssueOutcome::Failed)
        }
    }
}

/// Fall through to guided tool-error close, or hard-fail after too many incidents.
///
/// Does **not** reset `bad_tool_args_retry_count` — leaving it at the budget max
/// prevents another 2 automatic retries on every subsequent truncated attempt.
async fn fallthrough_or_hard_fail_malformed_tool_args(
    context: &RecoveryContext<'_>,
    session_id: &str,
    session_name: &str,
    reason: &NonProductiveCompletionReason,
    max_retries: u32,
) -> Result<StreamingIssueOutcome, String> {
    let NonProductiveCompletionReason::MalformedToolCallArguments {
        assistant_message_id,
        tool_names,
        parse_kind,
    } = reason
    else {
        return Ok(StreamingIssueOutcome::FallThrough);
    };

    let incident_count = {
        let active = context.active_sessions.read().await;
        let Some(session) = active.get(session_id) else {
            return Ok(StreamingIssueOutcome::FallThrough);
        };
        let mut incidents = session.bad_tool_args_incident_count.write().await;
        *incidents = incidents.saturating_add(1);
        *incidents
    };

    if incident_count > BAD_TOOL_ARGS_MAX_INCIDENTS {
        log::warn!(
            "Malformed/truncated tool-call arguments for session {} exceeded incident cap ({}/{}). Stopping workflow.",
            session_id,
            incident_count,
            BAD_TOOL_ARGS_MAX_INCIDENTS
        );
        finalize_workflow_error_with_dispatcher(
            context.session_repo,
            context.active_sessions,
            context.app_handle,
            session_id.to_string(),
            AgentRuntimeError::new(
                AgentRuntimeErrorType::AiServiceError,
                format!(
                    "The model repeatedly produced invalid tool-call arguments in session '{}' and exceeded the automatic recovery limit. Workflow stopped to prevent an infinite loop.",
                    session_name
                ),
            )
            .with_code("MALFORMED_TOOL_ARGS_LOOP")
            .with_original_error(serde_json::json!({
                "assistantMessageId": assistant_message_id,
                "toolNames": tool_names,
                "parseKind": parse_kind,
                "incidentCount": incident_count,
                "maxIncidents": BAD_TOOL_ARGS_MAX_INCIDENTS,
                "maxRetries": max_retries,
            })),
        )
        .await?;
        return Ok(StreamingIssueOutcome::Failed);
    }

    log::warn!(
        "Malformed/truncated tool-call arguments for session {} message {} (tools={:?}, kind={}, incident {}/{}). Falling through to guided tool-error close.",
        session_id,
        assistant_message_id,
        tool_names,
        parse_kind,
        incident_count,
        BAD_TOOL_ARGS_MAX_INCIDENTS
    );
    Ok(StreamingIssueOutcome::FallThrough)
}

fn repeated_loop_reason_from_report(
    issue_kind: StreamingIssueKind,
    report: &StreamingIssueReport,
) -> Option<NonProductiveCompletionReason> {
    match issue_kind {
        StreamingIssueKind::RepeatedThinkingLoop => {
            Some(NonProductiveCompletionReason::RepeatedThinkingLoop {
                response_message_id: report.response_message_id.clone(),
                observed_tail_chars: report.observed_tail_chars,
                pattern_length: report.pattern_length,
                repetition_count: report.repetition_count,
            })
        }
        StreamingIssueKind::RepeatedTextLoop => {
            Some(NonProductiveCompletionReason::RepeatedTextLoop {
                response_message_id: report.response_message_id.clone(),
                observed_tail_chars: report.observed_tail_chars,
                pattern_length: report.pattern_length,
                repetition_count: report.repetition_count,
            })
        }
        StreamingIssueKind::ReasoningBudgetExceeded => {
            Some(NonProductiveCompletionReason::ReasoningBudgetExceeded {
                response_message_id: report.response_message_id.clone(),
                observed_tail_chars: report.observed_tail_chars,
                reasoning_budget_tokens: report.pattern_length,
                estimated_thinking_tokens: report.repetition_count,
            })
        }
    }
}

fn recovery_counter_for_issue(issue_kind: StreamingIssueKind) -> StreamingRecoveryCounter {
    match issue_kind {
        StreamingIssueKind::RepeatedThinkingLoop => StreamingRecoveryCounter::Thinking,
        StreamingIssueKind::RepeatedTextLoop => StreamingRecoveryCounter::Text,
        StreamingIssueKind::ReasoningBudgetExceeded => StreamingRecoveryCounter::ReasoningBudget,
    }
}

fn cancel_reason_for_issue(issue_kind: StreamingIssueKind) -> &'static str {
    match issue_kind {
        StreamingIssueKind::RepeatedThinkingLoop => "repeated-thinking-loop",
        StreamingIssueKind::RepeatedTextLoop => "repeated-text-loop",
        StreamingIssueKind::ReasoningBudgetExceeded => "reasoning-budget-exceeded",
    }
}

fn stale_report_log_label(issue_kind: StreamingIssueKind) -> &'static str {
    match issue_kind {
        StreamingIssueKind::RepeatedThinkingLoop => "repeated-thinking",
        StreamingIssueKind::RepeatedTextLoop => "repeated-text",
        StreamingIssueKind::ReasoningBudgetExceeded => "reasoning-budget",
    }
}

pub async fn handle_streaming_issue(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    report: StreamingIssueReport,
) -> Result<StreamingIssueOutcome, String> {
    let issue_kind = report.issue_kind;
    let counter = recovery_counter_for_issue(issue_kind);
    let max_retries = counter.max_retries();
    let reason = repeated_loop_reason_from_report(issue_kind, &report)
        .ok_or_else(|| format!("Unsupported streaming issue kind: {:?}", issue_kind))?;

    let (action, session_name) = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(&report.session_id) else {
            return Ok(StreamingIssueOutcome::Ignored);
        };

        let expected_response_id = session.expected_response_id.read().await.clone();
        let retry_count = counter.read_count(session).await;
        let session_name = session
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| report.session_id[..8.min(report.session_id.len())].to_string());

        (
            evaluate_streaming_issue_action(
                expected_response_id.as_deref(),
                &report.response_message_id,
                retry_count,
                max_retries,
            ),
            session_name,
        )
    };

    if matches!(action, StreamingIssueAction::Ignore) {
        log::info!(
            "Ignoring stale {} report for session {} response {}",
            stale_report_log_label(issue_kind),
            report.session_id,
            report.response_message_id
        );
        return Ok(StreamingIssueOutcome::Ignored);
    }

    let previous_expected_response_id = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(&report.session_id) else {
            return Ok(StreamingIssueOutcome::Ignored);
        };

        let mut expected_response_id = session.expected_response_id.write().await;
        expected_response_id.take()
    };

    // Cancel must succeed before we increment the retry counter or start a new
    // completion. Otherwise the frontend stream may still be active and we would
    // risk dual completions or counting a retry that never stopped the loop.
    // See docs/architecture/text-loop-recovery.md (Recovery chain guarantees).
    if let Err(error) = emit_completion_cancel(
        app_handle,
        CompletionCancelRequest {
            session_id: report.session_id.clone(),
            response_message_id: report.response_message_id.clone(),
            reason: cancel_reason_for_issue(issue_kind).to_string(),
        },
    ) {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&report.session_id) {
            *session.expected_response_id.write().await = previous_expected_response_id;
        }

        return Err(error);
    }

    match action {
        StreamingIssueAction::Ignore => Ok(StreamingIssueOutcome::Ignored),
        StreamingIssueAction::CancelAndRetry { next_retry_count } => {
            handle_non_productive_completion(
                RecoveryContext {
                    session_repo,
                    active_sessions,
                    proxy_manager,
                    app_handle,
                },
                report.session_id,
                session_name,
                counter,
                NonProductiveCompletionAction::Retry { next_retry_count },
                reason,
            )
            .await
        }
        StreamingIssueAction::CancelAndFail => {
            handle_non_productive_completion(
                RecoveryContext {
                    session_repo,
                    active_sessions,
                    proxy_manager,
                    app_handle,
                },
                report.session_id,
                session_name,
                counter,
                NonProductiveCompletionAction::Fail,
                reason,
            )
            .await
        }
    }
}

pub async fn handle_thinking_only_completion(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    assistant_message_id: String,
) -> Result<StreamingIssueOutcome, String> {
    let counter = StreamingRecoveryCounter::Thinking;

    let (action, session_name) = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(&session_id) else {
            return Ok(StreamingIssueOutcome::Ignored);
        };

        let retry_count = counter.read_count(session).await;
        let session_name = session
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| session_id[..8.min(session_id.len())].to_string());

        (
            evaluate_non_productive_completion_action(retry_count, counter.max_retries()),
            session_name,
        )
    };

    handle_non_productive_completion(
        RecoveryContext {
            session_repo,
            active_sessions,
            proxy_manager,
            app_handle,
        },
        session_id,
        session_name,
        counter,
        action,
        NonProductiveCompletionReason::ThinkingOnlyCompletion {
            assistant_message_id,
        },
    )
    .await
}

/// Payload for a malformed/truncated tool-call-arguments recovery attempt.
pub struct MalformedToolArgsIncident {
    pub assistant_message_id: String,
    pub tool_names: Vec<String>,
    pub parse_kind: String,
}

/// Retry (or fall through after budget) when a completion contains tool calls
/// whose `function.arguments` are not valid JSON objects.
///
/// On `Retried`, the bad assistant message must **not** be cached — the caller
/// invokes this before `cache_assistant_message`. On `FallThrough`, the caller
/// continues with cache + guided tool-error close.
pub async fn handle_malformed_tool_args_completion(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    incident: MalformedToolArgsIncident,
) -> Result<StreamingIssueOutcome, String> {
    let counter = StreamingRecoveryCounter::BadToolArgs;

    let (action, session_name) = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(&session_id) else {
            return Ok(StreamingIssueOutcome::Ignored);
        };

        let retry_count = counter.read_count(session).await;
        let session_name = session
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| session_id[..8.min(session_id.len())].to_string());

        (
            evaluate_non_productive_completion_action(retry_count, counter.max_retries()),
            session_name,
        )
    };

    handle_non_productive_completion(
        RecoveryContext {
            session_repo,
            active_sessions,
            proxy_manager,
            app_handle,
        },
        session_id,
        session_name,
        counter,
        action,
        NonProductiveCompletionReason::MalformedToolCallArguments {
            assistant_message_id: incident.assistant_message_id,
            tool_names: incident.tool_names,
            parse_kind: incident.parse_kind,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_tool_args_retries_until_budget() {
        assert_eq!(
            evaluate_non_productive_completion_action(0, BAD_TOOL_ARGS_MAX_RETRIES),
            NonProductiveCompletionAction::Retry {
                next_retry_count: 1
            }
        );
        assert_eq!(
            evaluate_non_productive_completion_action(1, BAD_TOOL_ARGS_MAX_RETRIES),
            NonProductiveCompletionAction::Retry {
                next_retry_count: 2
            }
        );
        assert_eq!(
            evaluate_non_productive_completion_action(2, BAD_TOOL_ARGS_MAX_RETRIES),
            NonProductiveCompletionAction::Fail
        );
    }

    #[test]
    fn bad_tool_args_incident_cap_is_positive() {
        assert!(BAD_TOOL_ARGS_MAX_INCIDENTS >= 1);
        assert!(BAD_TOOL_ARGS_MAX_RETRIES >= 1);
    }

    #[test]
    fn reasoning_budget_retries_once() {
        assert_eq!(REASONING_BUDGET_MAX_RETRIES, 1);
        assert_eq!(
            evaluate_non_productive_completion_action(0, REASONING_BUDGET_MAX_RETRIES),
            NonProductiveCompletionAction::Retry {
                next_retry_count: 1
            }
        );
        assert_eq!(
            evaluate_non_productive_completion_action(1, REASONING_BUDGET_MAX_RETRIES),
            NonProductiveCompletionAction::Fail
        );
    }
}
