use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::agent::types::ToolCall;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::{SessionRepository, SessionStatus};
use agent_response_guards::is_internal_ui_callback_tool_name;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::completion::request_llm_completion;
use super::response_admission;
use super::response_circuit_breaker;
use super::tool_execution;
use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use crate::agent::tauri_events::TauriEventDispatcher;

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

struct AssistantMessageShape {
    has_content: bool,
    has_thinking: bool,
    has_tool_calls: bool,
}

fn inspect_assistant_message_shape(message: &Message) -> AssistantMessageShape {
    let has_content = message.content.iter().any(|content| match content {
        crate::mcp::types::MCPContent::Text { text, .. } => !text.trim().is_empty(),
        _ => true,
    });
    let has_thinking = message
        .thinking
        .as_ref()
        .map(|thinking| !thinking.is_empty())
        .unwrap_or(false);
    let has_tool_calls = message
        .tool_calls
        .as_ref()
        .map(|tool_calls| !tool_calls.is_empty())
        .unwrap_or(false);

    AssistantMessageShape {
        has_content,
        has_thinking,
        has_tool_calls,
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

fn assistant_message_has_only_internal_ui_callback_tool_calls(message: &Message) -> bool {
    message
        .tool_calls
        .as_ref()
        .map(|tool_calls| {
            !tool_calls.is_empty()
                && tool_calls.iter().all(|tool_call| {
                    is_internal_ui_callback_tool_name(tool_call.function.name.as_str())
                })
        })
        .unwrap_or(false)
}

async fn persist_assistant_message_to_db(message: &Message) {
    let repo = crate::state::get_message_repository();
    if let Err(error) = repo.insert(message).await {
        log::error!(
            "Failed to save assistant message to DB: msg_id={}, error={}",
            message.id,
            error
        );
    }
}

fn spawn_persist_assistant_message_to_db(message: Message) {
    tokio::spawn(async move {
        persist_assistant_message_to_db(&message).await;
    });
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

async fn reset_repeated_thinking_retry_count(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        *session.repeated_thinking_retry_count.write().await = 0;
    }
}

async fn session_has_pending_events(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> bool {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        return session.pending_events.read().await.count() > 0;
    }

    false
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
            results: Vec::new(),
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
    // Check cancellation and determine whether Idle tool-call entry is allowed
    let allow_idle_tool_entry = assistant_message
        .tool_calls
        .as_ref()
        .map(|calls| !calls.is_empty())
        .unwrap_or(false);
    let is_ui_tool = assistant_message_has_only_ui_tool_calls(&assistant_message);
    let is_internal_ui_callback =
        assistant_message_has_only_internal_ui_callback_tool_calls(&assistant_message);

    let admission = response_admission::inspect_response_admission(
        active_sessions,
        &session_id,
        allow_idle_tool_entry,
        is_ui_tool,
        is_internal_ui_callback,
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

    // [Circuit Breaker] Pre-process: Check for loops and inject circuit breaker if needed
    response_circuit_breaker::preprocess_assistant_tool_calls(
        active_sessions,
        &session_id,
        &mut assistant_message,
    )
    .await;

    // Check if content is also empty (abnormal empty response).
    // Note: A message with tool calls but no content is VALID and normal.
    // We check that at least one content item has meaningful text (matching
    // the frontend's hasContent logic), so that Gemini-style empty-text
    // responses like [{type:"text", text:""}] are not treated as valid content.
    let assistant_shape = inspect_assistant_message_shape(&assistant_message);

    if !assistant_shape.has_tool_calls {
        if !assistant_shape.has_content && !assistant_shape.has_thinking {
            // content, tool_calls, AND thinking are all empty - this is an error
            log::warn!(
                "⚠️  Empty LLM response detected for session {}: no content, tool calls, or thinking. This may indicate a model inference issue.",
                session_id
            );
            // Set status to error
            crate::agent::lifecycle::update_session_status(
                session_repo,
                active_sessions,
                app_handle,
                &session_id,
                SessionStatus::Error,
            )
            .await?;
            // Emit workflow error event with specific message
            let error_event = crate::agent::events::AgentEvent::WorkflowError {
                session_id: session_id.clone(),
                error: AgentRuntimeError::new(
                    AgentRuntimeErrorType::AiServiceError,
                    "The AI model returned an empty response with no content, tool calls, or thinking. This may indicate a model inference issue, context overflow, or generation failure. Please try again.",
                )
                .with_code("EMPTY_LLM_RESPONSE"),
            };
            crate::agent::tauri_events::emit_agent_event(app_handle, error_event)
                .map_err(|e| format!("Failed to emit WorkflowError event: {}", e))?;
            return Ok(());
        }

        if assistant_shape.has_thinking && !assistant_shape.has_content {
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
        reset_repeated_thinking_retry_count(active_sessions, &session_id).await;

        // Check for pending messages before finishing
        let has_pending = session_has_pending_events(active_sessions, &session_id).await;

        if has_pending {
            spawn_persist_assistant_message_to_db(msg_for_db);
            log::info!(
                "🔄 Pending messages detected for session {}. Continuing workflow.",
                session_id
            );
            // Recursively trigger next turn
            return request_llm_completion(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id,
            )
            .await
            .map(|_| ())
            .map_err(String::from);
        }

        // Ensure the final assistant row is visible before waking terminal waiters.
        persist_assistant_message_to_db(&msg_for_db).await;

        // No pending messages remain, so finish the workflow now.
        crate::agent::lifecycle::update_session_status(
            session_repo,
            active_sessions,
            app_handle,
            &session_id,
            SessionStatus::Idle,
        )
        .await?;

        let event = crate::agent::events::AgentEvent::WorkflowCompleted {
            session_id: session_id.clone(),
            reason: crate::agent::events::WorkflowCompletionReason::Natural,
        };
        crate::agent::tauri_events::emit_agent_event(app_handle, event)
            .map_err(|e| format!("Failed to emit event: {}", e))?;

        log::info!("Completed workflow for session: {}", session_id);
    } else {
        spawn_persist_assistant_message_to_db(msg_for_db);

        // Tools found! Initiate execution
        log::info!(
            "Processing {} tool calls for session: {}",
            tool_calls.len(),
            session_id
        );

        reset_repeated_thinking_retry_count(active_sessions, &session_id).await;

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
        // (e.g., writeFile followed by editFiles on the same file)
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

    let dispatcher = TauriEventDispatcher::new(app_handle.clone());
    finalize_workflow_error_with_dispatcher(
        session_repo,
        active_sessions,
        &dispatcher,
        session_id,
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
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
    error: AgentRuntimeError,
) -> Result<(), String> {
    crate::agent::lifecycle::update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        dispatcher,
        &session_id,
        SessionStatus::Error,
    )
    .await?;

    let event = AgentEvent::WorkflowError {
        session_id,
        error: error.clone(),
    };
    dispatcher.emit_agent_event(event)
}
