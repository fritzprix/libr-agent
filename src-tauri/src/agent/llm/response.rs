use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::agent::types::ToolCall;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::response_admission;
use super::response_circuit_breaker;
use super::tool_execution;
use crate::agent::llm::assistant_message_shape::inspect_assistant_message_shape;
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmErrorHandlingOutcome {
    RecoveredByCompaction,
    FinalizedWorkflowError,
}

pub fn completion_result_from_error_handling_outcome(
    outcome: LlmErrorHandlingOutcome,
    error: AgentRuntimeError,
) -> Result<(), String> {
    match outcome {
        LlmErrorHandlingOutcome::RecoveredByCompaction => Ok(()),
        LlmErrorHandlingOutcome::FinalizedWorkflowError => Err(error.into()),
    }
}

fn assistant_message_has_only_ui_tool_calls(message: &Message) -> bool {
    message
        .tool_calls
        .as_ref()
        .map(|tool_calls| {
            !tool_calls.is_empty()
                && tool_calls
                    .iter()
                    .all(|tool_call| tool_call.function.name.starts_with("ui__"))
        })
        .unwrap_or(false)
}

async fn persist_assistant_message_to_db(message: &Message) -> Result<(), String> {
    let repo = crate::state::get_message_repository();
    repo.insert(message).await.map_err(|error| {
        log::error!(
            "Failed to save assistant message to DB: msg_id={}, error={}",
            message.id,
            error
        );
        format!("Failed to save assistant message to DB: {}", error)
    })
}

async fn cache_assistant_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &Message,
) {
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut messages = session.messages.write().await;
        messages.push(assistant_message.clone());

        if messages.len() > MAX_CACHED_MESSAGES {
            let removed = messages.remove(0);
            log::debug!("Sliding window: evicted message {}", removed.id);
        }

        log::info!(
            "🤖 Message stack after assistant message: session={}, count={}, latest_message={}",
            session_id,
            messages.len(),
            assistant_message.id
        );
    }
}

fn extract_prompt_tokens(message: &Message) -> Option<i64> {
    message
        .usage
        .as_ref()
        .and_then(|usage| usage.get("promptTokens"))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
        })
}

async fn persist_prompt_token_checkpoint(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    prompt_tokens: i64,
) {
    let (checkpoint_handle, message_handle) = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return;
        };
        (
            session.last_submitted_input_message_id.clone(),
            session.messages.clone(),
        )
    };

    let checkpoint_id = checkpoint_handle.read().await.clone();
    let Some(checkpoint_id) = checkpoint_id else {
        return;
    };

    let checkpoint_message = {
        let mut messages = message_handle.write().await;
        let Some(message) = messages
            .iter_mut()
            .find(|message| message.id == checkpoint_id)
        else {
            log::warn!(
                "Failed to stamp prompt-token checkpoint: session={}, checkpoint_id={} missing from cache",
                session_id,
                checkpoint_id
            );
            return;
        };

        message.prompt_tokens = Some(prompt_tokens);
        message.clone()
    };

    let repo = crate::state::get_message_repository();
    if let Err(error) = repo.insert(&checkpoint_message).await {
        log::error!(
            "Failed to persist prompt-token checkpoint: session={}, checkpoint_id={}, error={}",
            session_id,
            checkpoint_message.id,
            error
        );
    }
}

async fn reset_streaming_recovery_retry_counts(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        *session.repeated_thinking_retry_count.write().await = 0;
        *session.repeated_text_loop_retry_count.write().await = 0;
        *session.reasoning_budget_retry_count.write().await = 0;
        // bad_tool_args_* counters are intentionally NOT reset here — they only
        // clear on workflow start or when a fully valid tool-call batch is admitted
        // (see reset_bad_tool_args_recovery_counts).
    }
}

async fn reset_bad_tool_args_recovery_counts(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        *session.bad_tool_args_retry_count.write().await = 0;
        *session.bad_tool_args_incident_count.write().await = 0;
    }
}

async fn initialize_pending_execution(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message_id: &str,
    tool_calls: &[ToolCall],
) {
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(session_id) {
        let expected_tool_call_ids: std::collections::HashSet<String> =
            tool_calls.iter().map(|tc| tc.id.clone()).collect();
        session.pending_execution = Some(crate::agent::state::PendingToolExecution {
            message_id: assistant_message_id.to_string(),
            total_expected: tool_calls.len(),
            tool_names: tool_calls
                .iter()
                .map(|tc| (tc.id.clone(), tc.function.name.clone()))
                .collect(),
            expected_tool_call_ids,
            completed_tool_call_ids: std::collections::HashSet::new(),
        });
    }
}

pub async fn initialize_pending_execution_for_testing(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message_id: &str,
    tool_calls: &[ToolCall],
) {
    initialize_pending_execution(
        active_sessions,
        session_id,
        assistant_message_id,
        tool_calls,
    )
    .await;
}

/// Handle an LLM response from the frontend
pub async fn handle_llm_response(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    mut assistant_message: Message,
) -> Result<(), String> {
    // Cold open (app restart → agent_open_session) activates the session without
    // hydrating the in-memory message cache. UI tool entry (e.g. resumeCircuitBreak)
    // comes through this path rather than start_workflow, so hydrate before dedup /
    // append — otherwise the next LLM turn would see only the injected tool call.
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    // Early return if this assistant message is a duplicate of the last message in the session cache
    let is_duplicate = {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            let session_messages = session.messages.read().await;
            if let Some(last_msg) = session_messages.last() {
                let last_sig = crate::services::message_service::message_signature(last_msg);
                let current_sig =
                    crate::services::message_service::message_signature(&assistant_message);
                last_sig.is_some() && last_sig == current_sig
            } else {
                false
            }
        } else {
            false
        }
    };

    if is_duplicate {
        log::info!(
            "Skipping duplicate LLM response message in session {}: msg_id={}",
            session_id,
            assistant_message.id
        );
        return Ok(());
    }

    // Check cancellation and determine whether Idle tool-call entry is allowed
    let allow_idle_tool_entry = assistant_message
        .tool_calls
        .as_ref()
        .map(|calls| !calls.is_empty())
        .unwrap_or(false);
    let is_ui_tool = assistant_message_has_only_ui_tool_calls(&assistant_message);

    let admission = response_admission::inspect_response_admission(
        active_sessions,
        &session_id,
        allow_idle_tool_entry,
        is_ui_tool,
    )
    .await?;

    if admission.should_mark_busy {
        response_admission::clear_cancel_pending_flag(active_sessions, &session_id).await;

        crate::agent::lifecycle::update_session_status(
            session_repo,
            active_sessions,
            app_handle,
            &session_id,
            SessionStatus::Busy,
        )
        .await?;

        let event = crate::agent::events::AgentEvent::WorkflowStarted {
            session_id: session_id.clone(),
        };
        crate::agent::tauri_events::emit_agent_event(app_handle, event)
            .map_err(|e| format!("Failed to emit WorkflowStarted event: {}", e))?;
    }

    // [Message ID Matching] Use pre-generated ID if available
    if !admission.skip_expected_response_id_check {
        response_admission::consume_expected_response_id(
            active_sessions,
            &session_id,
            &assistant_message.id,
            is_ui_tool,
        )
        .await?;
    }

    // [Bad tool-args] Validate BEFORE circuit-breaker preprocess so a retry
    // does not discard CB decisions / emit ghost CB events for a discarded turn.
    let assistant_shape_early = inspect_assistant_message_shape(&assistant_message);
    if assistant_shape_early.has_tool_calls {
        if let Some(tool_calls) = assistant_message.tool_calls.as_ref() {
            let invalid =
                crate::agent::llm::tool_args_validation::find_invalid_tool_call_args(tool_calls);
            if !invalid.is_empty() {
                let tool_names: Vec<String> = invalid.iter().map(|i| i.tool_name.clone()).collect();
                let parse_kind = invalid[0].kind.as_error_kind().to_string();
                match crate::agent::llm::stream_recovery::handle_malformed_tool_args_completion(
                    session_repo,
                    active_sessions,
                    proxy_manager,
                    app_handle,
                    session_id.clone(),
                    crate::agent::llm::stream_recovery::MalformedToolArgsIncident {
                        assistant_message_id: assistant_message.id.clone(),
                        tool_names,
                        parse_kind,
                    },
                )
                .await
                {
                    Ok(crate::agent::llm::stream_recovery::StreamingIssueOutcome::Retried {
                        ..
                    }) => {
                        // Bad completion discarded; a fresh LLM turn was requested.
                        return Ok(());
                    }
                    Ok(crate::agent::llm::stream_recovery::StreamingIssueOutcome::Failed) => {
                        // Workflow already finalized with an error.
                        return Ok(());
                    }
                    Ok(
                        crate::agent::llm::stream_recovery::StreamingIssueOutcome::FallThrough
                        | crate::agent::llm::stream_recovery::StreamingIssueOutcome::Ignored,
                    ) => {
                        // Fall through: cache + execute so guided parse-error closes each tool call.
                    }
                    Err(error) => {
                        // Defensive: if recovery plumbing fails, still close tools.
                        log::warn!(
                            "Malformed tool-args recovery returned error for session {}: {}. Falling through to guided close.",
                            session_id,
                            error
                        );
                    }
                }
            } else {
                // Clean valid batch — clear the per-workflow malformed budget.
                reset_bad_tool_args_recovery_counts(active_sessions, &session_id).await;
            }
        }
    }

    // [Circuit Breaker] Pre-process: Check for loops and inject circuit breaker if needed
    let circuit_breaker_preprocess = response_circuit_breaker::preprocess_assistant_tool_calls(
        active_sessions,
        &session_id,
        &mut assistant_message,
    )
    .await;
    let loop_prevention_short_circuits = circuit_breaker_preprocess.loop_prevention_short_circuits;

    // Check if content is also empty (abnormal empty response).
    // Note: A message with tool calls but no content is VALID and normal.
    // We check that at least one content item has meaningful text (matching
    // the frontend's hasContent logic), so that Gemini-style empty-text
    // responses like [{type:"text", text:""}] are not treated as valid content.
    let assistant_shape = inspect_assistant_message_shape(&assistant_message);

    if !assistant_shape.has_tool_calls {
        if !assistant_shape.has_renderable_content && !assistant_shape.has_thinking {
            // content, tool_calls, AND thinking are all empty - this is an error
            log::warn!(
                "⚠️  Empty LLM response detected for session {}: no content, tool calls, or thinking. This may indicate a model inference issue.",
                session_id
            );
            let runtime_error = AgentRuntimeError::new(
                AgentRuntimeErrorType::AiServiceError,
                "The AI model returned an empty response with no content, tool calls, or thinking. This may indicate a model inference issue, context overflow, or generation failure. Please try again.",
            )
            .with_code("EMPTY_LLM_RESPONSE");
            crate::agent::workflow::settle_session_and_finalize_error(
                session_repo,
                active_sessions,
                app_handle,
                &session_id,
                None,
                runtime_error,
            )
            .await?;
            return Ok(());
        }

        if assistant_shape.is_thinking_only_completion() {
            return crate::agent::llm::stream_recovery::handle_thinking_only_completion(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id.clone(),
                assistant_message.id.clone(),
            )
            .await
            .map(|_| ());
        }
    }

    // [Race Mitigation] Verify session has not been reset/cancelled during async yields
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            if session.cancellation_token.is_cancelled() {
                log::info!(
                    "Workflow was cancelled or reset for session {} after admission. Discarding LLM response.",
                    session_id
                );
                return Ok(());
            }
        } else {
            return Err(format!("Session not found: {}", session_id));
        }
    }

    if let Some(prompt_tokens) = extract_prompt_tokens(&assistant_message) {
        persist_prompt_token_checkpoint(active_sessions, &session_id, prompt_tokens).await;
    }

    // 1. Add assistant message to cache
    cache_assistant_message(active_sessions, &session_id, &assistant_message).await;

    // 2. Emit MessageAdded event
    let message_added_event = crate::agent::events::AgentEvent::MessageAdded {
        session_id: session_id.clone(),
        message: Box::new(assistant_message.clone()),
    };
    crate::agent::tauri_events::emit_agent_event(app_handle, message_added_event)
        .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;

    let msg_for_db = assistant_message.clone();

    // Parse tool calls
    // ⚡ Bolt: Use take() to move the vector out of the struct instead of cloning it, saving a deep copy.
    let tool_calls: Vec<ToolCall> = assistant_message.tool_calls.take().unwrap_or_default();

    if tool_calls.is_empty() {
        reset_streaming_recovery_retry_counts(active_sessions, &session_id).await;
        reset_bad_tool_args_recovery_counts(active_sessions, &session_id).await;

        if crate::agent::workflow::session_has_pending_events(active_sessions, &session_id).await {
            persist_assistant_message_to_db(&msg_for_db).await?;
            crate::agent::workflow::continue_workflow_if_pending_events(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                &session_id,
            )
            .await?;
            return Ok(());
        }

        let restarted = crate::agent::workflow::settle_session_and_go_idle(
            session_repo,
            active_sessions,
            proxy_manager,
            app_handle,
            &session_id,
            Some(&msg_for_db),
            crate::agent::events::WorkflowCompletionReason::Natural,
        )
        .await?;

        if !restarted {
            log::info!("Completed workflow for session: {}", session_id);
        }
    } else {
        persist_assistant_message_to_db(&msg_for_db).await?;

        // Tools found! Initiate execution
        log::info!(
            "Processing {} tool calls for session: {}",
            tool_calls.len(),
            session_id
        );

        reset_streaming_recovery_retry_counts(active_sessions, &session_id).await;

        // Update status to Busy
        crate::agent::lifecycle::update_session_status(
            session_repo,
            active_sessions,
            app_handle,
            &session_id,
            SessionStatus::Busy,
        )
        .await?;

        // Initialize pending execution state
        initialize_pending_execution(
            active_sessions,
            &session_id,
            &assistant_message.id,
            &tool_calls,
        )
        .await;

        // Execute tool calls
        // 🔥 CRITICAL CHANGE: Execute tools SEQUENTIALLY to prevent race conditions
        // (e.g., writeFile followed by editFile on the same file)
        let session_repo_clone = session_repo.clone();
        let active_sessions_clone = active_sessions.clone();
        let app_handle_clone = app_handle.clone();
        let proxy_manager_clone = proxy_manager.clone();
        let session_id_clone = session_id.clone();

        // ⚡ Bolt: Move tool_calls into the async block instead of cloning it.
        tokio::spawn(async move {
            tool_execution::execute_tool_calls(
                session_repo_clone,
                active_sessions_clone,
                proxy_manager_clone,
                app_handle_clone,
                session_id_clone,
                tool_calls,
                loop_prevention_short_circuits,
            )
            .await;
        });
    }

    Ok(())
}

/// Handle LLM error from frontend
pub async fn handle_llm_error_with_outcome(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    error: AgentRuntimeError,
) -> Result<LlmErrorHandlingOutcome, String> {
    log::error!(
        "LLM error for session {}: {}",
        session_id,
        error.display_message
    );

    let context_settings = crate::agent::llm::completion::load_context_management_settings().await;
    let context_strategy = context_settings.context_strategy().to_string();

    if matches!(
        error.error_type,
        crate::agent::llm::types::AgentRuntimeErrorType::ContextLimitError
    ) && crate::agent::llm::completion::uses_compaction_strategy(&context_strategy)
    {
        match crate::agent::llm::trigger_preflight_compaction_for_session(
            active_sessions,
            app_handle,
            &session_id,
        )
        .await
        {
            Ok(true) => {
                log::info!(
                    "Recovered context-limit error by arming compaction recovery for session {}",
                    session_id
                );
                return Ok(LlmErrorHandlingOutcome::RecoveredByCompaction);
            }
            Ok(false) => {
                log::warn!(
                    "Context-limit error could not trigger compaction recovery for session {}",
                    session_id
                );
            }
            Err(compaction_error) => {
                log::error!(
                    "Failed to trigger compaction recovery after context-limit error for session {}: {}",
                    session_id,
                    compaction_error
                );
            }
        }
    } else if matches!(
        error.error_type,
        crate::agent::llm::types::AgentRuntimeErrorType::ContextLimitError
    ) {
        log::warn!(
            "Context-limit error will not trigger compaction recovery because strategy={} for session {}",
            context_strategy,
            session_id
        );
    }

    crate::agent::workflow::settle_session_and_finalize_error(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        None,
        error,
    )
    .await?;

    Ok(LlmErrorHandlingOutcome::FinalizedWorkflowError)
}

pub async fn handle_llm_error(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    error: AgentRuntimeError,
) -> Result<(), String> {
    handle_llm_error_with_outcome(session_repo, active_sessions, app_handle, session_id, error)
        .await
        .map(|_| ())
}

pub async fn finalize_workflow_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    error: AgentRuntimeError,
) -> Result<(), String> {
    crate::agent::workflow::settle_session_and_finalize_error(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        None,
        error,
    )
    .await
}
