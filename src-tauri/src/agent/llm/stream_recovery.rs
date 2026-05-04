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

pub fn evaluate_streaming_issue_action(
    expected_response_id: Option<&str>,
    reported_response_id: &str,
    retry_count: u32,
) -> StreamingIssueAction {
    if expected_response_id != Some(reported_response_id) {
        return StreamingIssueAction::Ignore;
    }

    if retry_count < REPEATED_THINKING_MAX_RETRIES {
        return StreamingIssueAction::CancelAndRetry {
            next_retry_count: retry_count + 1,
        };
    }

    StreamingIssueAction::CancelAndFail
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

    emit_completion_cancel(
        app_handle,
        CompletionCancelRequest {
            session_id: report.session_id.clone(),
            response_message_id: report.response_message_id.clone(),
            reason: "repeated-thinking-loop".to_string(),
        },
    )?;

    match action {
        StreamingIssueAction::Ignore => Ok(StreamingIssueOutcome::Ignored),
        StreamingIssueAction::CancelAndRetry { next_retry_count } => {
            {
                let active = active_sessions.read().await;
                if let Some(session) = active.get(&report.session_id) {
                    *session.repeated_thinking_retry_count.write().await = next_retry_count;
                }
            }

            log::warn!(
                "Repeated thinking loop detected for session {} response {} (tail_chars={}, pattern_length={}, repetition_count={}). Retrying LLM turn ({}/{}).",
                report.session_id,
                report.response_message_id,
                report.observed_tail_chars,
                report.pattern_length,
                report.repetition_count,
                next_retry_count,
                REPEATED_THINKING_MAX_RETRIES
            );

            request_llm_completion_with_recovery(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                report.session_id.clone(),
            )
            .await?;

            Ok(StreamingIssueOutcome::Retried {
                retry_count: next_retry_count,
            })
        }
        StreamingIssueAction::CancelAndFail => {
            {
                let active = active_sessions.read().await;
                if let Some(session) = active.get(&report.session_id) {
                    *session.expected_response_id.write().await = None;
                    *session.repeated_thinking_retry_count.write().await = 0;
                }
            }

            let dispatcher = TauriEventDispatcher::new(app_handle.clone());
            finalize_workflow_error_with_dispatcher(
                session_repo,
                active_sessions,
                &dispatcher,
                report.session_id.clone(),
                AgentRuntimeError::new(
                    AgentRuntimeErrorType::AiServiceError,
                    format!(
                        "The model got stuck repeating thinking content in session '{}' and exceeded the automatic recovery limit. Workflow stopped to prevent an infinite loop.",
                        session_name
                    ),
                )
                .with_code("REPEATED_THINKING_LOOP")
                .with_original_error(serde_json::json!({
                    "responseMessageId": report.response_message_id,
                    "observedTailChars": report.observed_tail_chars,
                    "patternLength": report.pattern_length,
                    "repetitionCount": report.repetition_count,
                    "maxRetries": REPEATED_THINKING_MAX_RETRIES,
                })),
            )
            .await?;

            Ok(StreamingIssueOutcome::Failed)
        }
    }
}
