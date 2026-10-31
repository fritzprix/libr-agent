use super::constants::{BAD_TOOL_ARGS_MAX_INCIDENTS, MALFORMED_TOOL_MARKUP_NOTE};
use super::counter::StreamingRecoveryCounter;
use super::types::{
    NonProductiveCompletionAction, NonProductiveCompletionReason, RecoveryContext,
    StreamingIssueAction, StreamingIssueOutcome,
};
use crate::agent::llm::completion::request_llm_completion_with_recovery;
use crate::agent::llm::response::finalize_workflow_error_with_dispatcher;
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};

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

pub(super) async fn handle_non_productive_completion(
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
                NonProductiveCompletionReason::MalformedToolMarkup {
                    assistant_message_id,
                    tool_name,
                } => {
                    log::warn!(
                        "Text-formatted tool-call markup for '{}' ended session {} message {}. Discarding it and retrying LLM turn ({}/{}).",
                        tool_name,
                        session_id,
                        assistant_message_id,
                        next_retry_count,
                        max_retries
                    );
                    append_recovery_note(&context, &session_id, MALFORMED_TOOL_MARKUP_NOTE).await?;
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
                NonProductiveCompletionReason::MalformedToolMarkup { .. }
            ) {
                // Budget spent: end the way it does without recovery (no new error).
                log::warn!(
                    "Malformed tool-call markup persisted after {} retries for session {}. Ending the turn as before.",
                    max_retries,
                    session_id
                );
                return Ok(StreamingIssueOutcome::FallThrough);
            }

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
                        "Stopped: the model returned only internal reasoning with no answer or tool call (session '{}'). Automatic recovery was exhausted. Click Retry to try again.",
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
                NonProductiveCompletionReason::MalformedToolMarkup { .. } => {
                    unreachable!("MalformedToolMarkup Fail handled above")
                }
                NonProductiveCompletionReason::ReasoningBudgetExceeded {
                    response_message_id,
                    observed_tail_chars,
                    reasoning_budget_tokens,
                    estimated_thinking_tokens,
                } => (
                    format!(
                        "Stopped: reasoning used the full output budget twice without an answer or tool call (session '{}'). This prevents a hang. Click Retry to try again.",
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

/// Appends a hidden recovery note (user role) to the history before a retry.
async fn append_recovery_note(
    context: &RecoveryContext<'_>,
    session_id: &str,
    text: &str,
) -> Result<(), String> {
    let note = crate::models::chat::Message::new_user_message(
        session_id.to_string(),
        text.to_string(),
        Some(crate::models::chat::MessageSource::Recovery),
        None,
    );
    {
        let active = context.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let mut messages = session.messages.write().await;
            messages.push(note.clone());
            crate::agent::state::enforce_cached_message_limit(&mut messages);
        }
    }
    use crate::repositories::message_repository::MessageRepository as _;
    crate::state::get_message_repository()
        .insert(&note)
        .await
        .map_err(|error| format!("Failed to save recovery note: {error}"))?;
    crate::agent::tauri_events::emit_agent_event(
        context.app_handle,
        crate::agent::events::AgentEvent::MessageAdded {
            session_id: session_id.to_string(),
            message: Box::new(note),
        },
    )
    .map_err(|error| format!("Failed to emit MessageAdded for recovery note: {error}"))
}
