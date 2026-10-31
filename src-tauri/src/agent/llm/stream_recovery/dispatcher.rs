use super::counter::StreamingRecoveryCounter;
use super::handlers::{
    evaluate_non_productive_completion_action, evaluate_streaming_issue_action,
    handle_non_productive_completion,
};
use super::types::{
    MalformedToolArgsIncident, NonProductiveCompletionAction, NonProductiveCompletionReason,
    RecoveryContext, StreamingIssueAction, StreamingIssueOutcome,
};
use crate::agent::llm::types::{CompletionCancelRequest, StreamingIssueKind, StreamingIssueReport};
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::emit_completion_cancel;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

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

/// A final turn that is only `<function=NAME>…` text: the caller has already
/// discarded the message (it is neither cached nor persisted). Retries up to
/// [`MALFORMED_TOOL_MARKUP_MAX_RETRIES`] times with a corrective note; after
/// that returns `FallThrough` so the turn ends exactly as it does today.
pub async fn handle_malformed_tool_markup_completion(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    assistant_message_id: String,
    tool_name: String,
) -> Result<StreamingIssueOutcome, String> {
    let counter = StreamingRecoveryCounter::MalformedToolMarkup;

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
        NonProductiveCompletionReason::MalformedToolMarkup {
            assistant_message_id,
            tool_name,
        },
    )
    .await
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
