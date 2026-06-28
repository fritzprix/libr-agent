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

async fn reset_streaming_recovery_retry_counts(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        *session.repeated_thinking_retry_count.write().await = 0;
        *session.repeated_text_loop_retry_count.write().await = 0;
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

    // [Message Loop Check] Drop and resubmit immediately if repetitive loop is detected
    if let Some(loop_action) = check_and_handle_message_loop(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        &assistant_message,
    )
    .await?
    {
        match loop_action {
            AssistantMessageLoopAction::Resubmitted => {
                crate::agent::llm::completion::request_llm_completion_with_recovery(
                    session_repo,
                    active_sessions,
                    proxy_manager,
                    app_handle,
                    session_id.to_string(),
                )
                .await?;
                return Ok(());
            }
            AssistantMessageLoopAction::Aborted => {
                return Ok(());
            }
        }
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

        if crate::agent::workflow::session_has_pending_events(active_sessions, &session_id).await {
            spawn_persist_assistant_message_to_db(msg_for_db);
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

        // Ensure the final assistant row is visible before waking terminal waiters.
        persist_assistant_message_to_db(&msg_for_db).await;

        if crate::agent::workflow::continue_workflow_if_pending_events(
            session_repo,
            active_sessions,
            proxy_manager,
            app_handle,
            &session_id,
        )
        .await?
        {
            return Ok(());
        }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssistantMessageLoopAction {
    Resubmitted,
    Aborted,
}

async fn check_and_handle_message_loop(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    assistant_message: &Message,
) -> Result<Option<AssistantMessageLoopAction>, String> {
    let current_sig = crate::services::message_service::message_signature(assistant_message);
    let Some(current_sig) = current_sig else {
        return Ok(None);
    };

    let is_duplicate = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return Ok(None);
        };

        let messages = session.messages.read().await;
        let mut duplicate = false;

        // Compare with the last 5 messages from the assistant in the sliding window
        for msg in messages
            .iter()
            .rev()
            .filter(|m| m.role == "assistant")
            .take(5)
        {
            if let Some(sig) = crate::services::message_service::message_signature(msg) {
                if sig == current_sig {
                    duplicate = true;
                    break;
                }
            }
        }
        duplicate
    };

    if is_duplicate {
        let (retry_count, max_retries) = {
            let sessions = active_sessions.read().await;
            let Some(session) = sessions.get(session_id) else {
                return Ok(None);
            };
            let count = *session.repeated_text_loop_retry_count.read().await;
            let threshold =
                crate::agent::llm::circuit_breaker::load_loop_prevention_threshold().await;
            (count as usize, threshold)
        };

        if retry_count >= max_retries {
            log::error!(
                "Repetitive loop detected and exceeded max retries ({}/{}) for session {}. Aborting workflow.",
                retry_count, max_retries, session_id
            );

            {
                let sessions = active_sessions.read().await;
                if let Some(session) = sessions.get(session_id) {
                    *session.repeated_text_loop_retry_count.write().await = 0;
                }
            }

            let dispatcher = TauriEventDispatcher::new(app_handle.clone());
            finalize_workflow_error_with_dispatcher(
                session_repo,
                active_sessions,
                &dispatcher,
                session_id.to_string(),
                AgentRuntimeError::new(
                    AgentRuntimeErrorType::AiServiceError,
                    "The agent got stuck in a repetitive response loop and was stopped to prevent infinite execution. Please refine your instruction.",
                )
                .with_code("REPETITIVE_LOOP_DETECTED"),
            )
            .await?;

            return Ok(Some(AssistantMessageLoopAction::Aborted));
        } else {
            {
                let sessions = active_sessions.read().await;
                if let Some(session) = sessions.get(session_id) {
                    *session.repeated_text_loop_retry_count.write().await =
                        (retry_count + 1) as u32;
                }
            }

            log::warn!(
                "Detected duplicate assistant message in session {}. Dropping response and triggering resubmit ({}/{})...",
                session_id, retry_count + 1, max_retries
            );

            return Ok(Some(AssistantMessageLoopAction::Resubmitted));
        }
    }

    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            *session.repeated_text_loop_retry_count.write().await = 0;
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::AgentSession;
    use crate::mcp::types::MCPContent;
    use crate::repositories::session_repository::MockSessionRepository;
    use crate::repositories::SessionMetadata;
    use std::sync::atomic::AtomicBool;
    use tauri::test::MockRuntime;
    use tauri_mcp_agent_lib_derive::AgentEventDispatcher;
    use tokio_util::sync::CancellationToken;

    fn build_test_session_metadata(session_id: &str) -> SessionMetadata {
        SessionMetadata {
            id: session_id.to_string(),
            name: Some("test-session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            assistant_id: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
            created_at: 0,
            updated_at: 0,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: crate::agent::ExecutionMode::Normal,
            workspace_override: None,
        }
    }

    fn build_test_agent_session(session_id: &str, messages: Vec<Message>) -> AgentSession {
        AgentSession {
            metadata: build_test_session_metadata(session_id),
            is_running: false,
            active_permit: None,
            status_transition: Arc::new(RwLock::new(None)),
            transition_lock: Arc::new(tokio::sync::Mutex::new(())),
            cancellation_token: CancellationToken::new(),
            yolo_mode: Arc::new(AtomicBool::new(false)),
            unsafe_mode: Arc::new(AtomicBool::new(false)),
            cancel_pending: Arc::new(AtomicBool::new(false)),
            pending_execution: None,
            messages: Arc::new(RwLock::new(messages)),
            cache_initialized: Arc::new(AtomicBool::new(true)),
            last_synced_at: Arc::new(RwLock::new(None)),
            repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
            repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
            pending_events: Arc::new(RwLock::new(crate::agent::state::PendingEventManager::new())),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            context_registry: Arc::new(crate::agent::context::registry::ContextRegistry::new()),
            compact_context: Arc::new(RwLock::new(None)),
            compaction: crate::agent::state::CompactionRuntimeState::new(),
            expected_response_id: Arc::new(RwLock::new(None)),
            cached_stable_prompt: Arc::new(RwLock::new(None)),
            last_completion_request: Arc::new(RwLock::new(None)),
            last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        }
    }

    fn build_assistant_message(id: &str, text: &str) -> Message {
        Message {
            id: id.to_string(),
            session_id: "test-session".to_string(),
            role: "assistant".to_string(),
            content: vec![MCPContent::Text {
                text: text.to_string(),
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: Some(false),
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            prompt_tokens: None,
            created_at: 0,
            updated_at: 0,
            source: None,
            error: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_check_and_handle_message_loop_no_duplicate() {
        let session_id = "test-session-1";
        let session_repo = Arc::new(MockSessionRepository::new()) as Arc<dyn SessionRepository>;
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));

        let history = vec![build_assistant_message("msg-1", "Hello world")];
        active_sessions.write().await.insert(
            session_id.to_string(),
            build_test_agent_session(session_id, history),
        );

        let mock_app = tauri::test::mock_app();
        let mock_handle = mock_app.handle();
        let app_handle: &tauri::AppHandle = unsafe {
            &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
        };

        let current_msg = build_assistant_message("msg-2", "Different message");
        let result = check_and_handle_message_loop(
            &session_repo,
            &active_sessions,
            app_handle,
            session_id,
            &current_msg,
        )
        .await
        .unwrap();

        assert!(result.is_none());

        // Counter must be 0
        let active = active_sessions.read().await;
        let session = active.get(session_id).unwrap();
        assert_eq!(*session.repeated_text_loop_retry_count.read().await, 0);
    }

    #[tokio::test]
    async fn test_check_and_handle_message_loop_duplicate_triggers_resubmit() {
        let session_id = "test-session-2";
        let session_repo = Arc::new(MockSessionRepository::new()) as Arc<dyn SessionRepository>;
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));

        let history = vec![build_assistant_message("msg-1", "Repeat me")];
        active_sessions.write().await.insert(
            session_id.to_string(),
            build_test_agent_session(session_id, history),
        );

        let mock_app = tauri::test::mock_app();
        let mock_handle = mock_app.handle();
        let app_handle: &tauri::AppHandle = unsafe {
            &*(mock_handle as *const tauri::AppHandle<MockRuntime> as *const tauri::AppHandle)
        };

        let current_msg = build_assistant_message("msg-2", "Repeat me");
        let result = check_and_handle_message_loop(
            &session_repo,
            &active_sessions,
            app_handle,
            session_id,
            &current_msg,
        )
        .await
        .unwrap();

        assert_eq!(result, Some(AssistantMessageLoopAction::Resubmitted));

        // Counter must increment to 1
        let active = active_sessions.read().await;
        let session = active.get(session_id).unwrap();
        assert_eq!(*session.repeated_text_loop_retry_count.read().await, 1);
    }
}
