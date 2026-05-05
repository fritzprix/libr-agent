use crate::agent::llm::completion::request_llm_completion_with_recovery;
use crate::agent::llm::response::finalize_workflow_error_with_dispatcher;
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompletionCancelRequest, StreamingIssueKind,
    StreamingIssueReport,
};
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::{emit_completion_cancel, TauriEventDispatcher};
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub const REPEATED_THINKING_MAX_RETRIES: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingIssueAction {
    Ignore,
    CancelAndRetry { next_retry_count: u32 },
    CancelAndFail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingIssueOutcome {
    Ignored,
    Retried { retry_count: u32 },
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
    ThinkingOnlyCompletion {
        assistant_message_id: String,
    },
}

struct RecoveryContext<'a> {
    session_repo: &'a Arc<dyn SessionRepository>,
    active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &'a Arc<MCPServiceProxyManager>,
    app_handle: &'a AppHandle,
}

pub fn evaluate_non_productive_completion_action(
    retry_count: u32,
) -> NonProductiveCompletionAction {
    if retry_count < REPEATED_THINKING_MAX_RETRIES {
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
) -> StreamingIssueAction {
    if expected_response_id != Some(reported_response_id) {
        return StreamingIssueAction::Ignore;
    }

    match evaluate_non_productive_completion_action(retry_count) {
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
    action: NonProductiveCompletionAction,
    reason: NonProductiveCompletionReason,
) -> Result<StreamingIssueOutcome, String> {
    match action {
        NonProductiveCompletionAction::Retry { next_retry_count } => {
            {
                let active = context.active_sessions.read().await;
                if let Some(session) = active.get(&session_id) {
                    *session.repeated_thinking_retry_count.write().await = next_retry_count;
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
                        REPEATED_THINKING_MAX_RETRIES
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
                        REPEATED_THINKING_MAX_RETRIES
                    );
                }
            }

            request_llm_completion_with_recovery(
                context.session_repo,
                context.active_sessions,
                context.proxy_manager,
                context.app_handle,
                session_id,
            )
            .await?;

            Ok(StreamingIssueOutcome::Retried {
                retry_count: next_retry_count,
            })
        }
        NonProductiveCompletionAction::Fail => {
            {
                let active = context.active_sessions.read().await;
                if let Some(session) = active.get(&session_id) {
                    *session.repeated_thinking_retry_count.write().await = 0;
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
                        "maxRetries": REPEATED_THINKING_MAX_RETRIES,
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
                        "maxRetries": REPEATED_THINKING_MAX_RETRIES,
                    }),
                ),
            };

            let dispatcher = TauriEventDispatcher::new(context.app_handle.clone());
            finalize_workflow_error_with_dispatcher(
                context.session_repo,
                context.active_sessions,
                &dispatcher,
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

pub async fn handle_streaming_issue(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    report: StreamingIssueReport,
) -> Result<StreamingIssueOutcome, String> {
    if report.issue_kind != StreamingIssueKind::RepeatedThinkingLoop {
        return Ok(StreamingIssueOutcome::Ignored);
    }

    let (action, session_name) = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(&report.session_id) else {
            return Ok(StreamingIssueOutcome::Ignored);
        };

        let expected_response_id = session.expected_response_id.read().await.clone();
        let retry_count = *session.repeated_thinking_retry_count.read().await;
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
            ),
            session_name,
        )
    };

    if matches!(action, StreamingIssueAction::Ignore) {
        log::info!(
            "Ignoring stale repeated-thinking report for session {} response {}",
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

    if let Err(error) = emit_completion_cancel(
        app_handle,
        CompletionCancelRequest {
            session_id: report.session_id.clone(),
            response_message_id: report.response_message_id.clone(),
            reason: "repeated-thinking-loop".to_string(),
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
                NonProductiveCompletionAction::Retry { next_retry_count },
                NonProductiveCompletionReason::RepeatedThinkingLoop {
                    response_message_id: report.response_message_id,
                    observed_tail_chars: report.observed_tail_chars,
                    pattern_length: report.pattern_length,
                    repetition_count: report.repetition_count,
                },
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
                NonProductiveCompletionAction::Fail,
                NonProductiveCompletionReason::RepeatedThinkingLoop {
                    response_message_id: report.response_message_id,
                    observed_tail_chars: report.observed_tail_chars,
                    pattern_length: report.pattern_length,
                    repetition_count: report.repetition_count,
                },
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
    let (action, session_name) = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(&session_id) else {
            return Ok(StreamingIssueOutcome::Ignored);
        };

        let retry_count = *session.repeated_thinking_retry_count.read().await;
        let session_name = session
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| session_id[..8.min(session_id.len())].to_string());

        (
            evaluate_non_productive_completion_action(retry_count),
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
        action,
        NonProductiveCompletionReason::ThinkingOnlyCompletion {
            assistant_message_id,
        },
    )
    .await
}
