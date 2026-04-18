use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::session_repository::SessionAttentionReason;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::circuit_breaker;
use super::completion::{request_llm_completion, trigger_post_response_compaction_if_needed};
use super::tool_execution;
use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, PostResponseCompactionPressure,
};
use crate::agent::state::DeferredWorkflowStep;
use crate::agent::tauri_events::TauriEventDispatcher;

async fn calculate_post_response_compaction_pressure(
    assistant_message: &Message,
) -> Option<PostResponseCompactionPressure> {
    let total_tokens = crate::agent::llm::token_utils::calculate_post_response_compaction_tokens(
        assistant_message,
    )?;
    let settings = crate::agent::llm::completion::load_context_management_settings().await;
    if !crate::agent::llm::completion::uses_compaction_strategy(&settings.context_strategy) {
        return None;
    }

    let context_window = std::cmp::min(settings.max_input_context, settings.model_max_limit);
    Some(PostResponseCompactionPressure {
        total_tokens,
        context_window,
        model_max_context: settings.model_max_limit,
    })
}

async fn defer_for_post_response_compaction_if_needed(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    post_response_compaction_pressure: Option<&PostResponseCompactionPressure>,
    deferred_step: DeferredWorkflowStep,
) -> Result<bool, String> {
    let Some(post_response_compaction_pressure) = post_response_compaction_pressure else {
        return Ok(false);
    };

    let (message_snapshot, session_name) = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let session_name = session
                .metadata
                .name
                .clone()
                .unwrap_or_else(|| session_id[..8.min(session_id.len())].to_string());
            let message_snapshot = session.messages.read().await.clone();
            (message_snapshot, session_name)
        } else {
            (
                Vec::new(),
                session_id[..8.min(session_id.len())].to_string(),
            )
        }
    };

    if message_snapshot.is_empty() {
        return Ok(false);
    }

    trigger_post_response_compaction_if_needed(
        active_sessions,
        app_handle,
        session_id,
        &session_name,
        &message_snapshot,
        post_response_compaction_pressure.total_tokens,
        deferred_step,
    )
    .await
}

/// Handle an LLM response from the frontend
pub async fn handle_llm_response(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    mut assistant_message: Message,
) -> Result<Option<PostResponseCompactionPressure>, String> {
    // Check cancellation and determine whether Idle tool-call entry is allowed
    let allow_idle_tool_entry = assistant_message
        .tool_calls
        .as_ref()
        .map(|calls| !calls.is_empty())
        .unwrap_or(false);

    let mut should_mark_busy = false;
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            let token_cancelled = session.cancellation_token.is_cancelled();
            let cancel_pending = session.cancel_pending.load(Ordering::SeqCst);
            let status = session.metadata.status.clone();

            if token_cancelled || cancel_pending {
                log::info!(
                    "Workflow cancelled for session: {} (token_cancelled={}, cancel_pending={}, status={:?})",
                    session_id,
                    token_cancelled,
                    cancel_pending,
                    status
                );
                return Err("Workflow was cancelled".to_string());
            }

            if status == SessionStatus::Busy {
                // Normal path while workflow is already running
            } else if status == SessionStatus::Idle && allow_idle_tool_entry {
                // Allow tool-call initiated workflow start from Idle
                should_mark_busy = true;
            } else {
                log::info!(
                    "Rejecting LLM response for session {} (status={:?}, has_tool_calls={})",
                    session_id,
                    status,
                    allow_idle_tool_entry
                );
                return Err("Workflow was cancelled".to_string());
            }
        }
    }

    if should_mark_busy {
        {
            let active = active_sessions.read().await;
            if let Some(session) = active.get(&session_id) {
                session.cancel_pending.store(false, Ordering::SeqCst);
            }
        }

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
    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            let mut expected_id = session.expected_response_id.write().await;
            if let Some(id) = expected_id.take() {
                assistant_message.id = id;
            }
        }
    }

    // [Circuit Breaker] Pre-process: Check for loops and inject circuit breaker if needed
    let mut forced_circuit_break_message: Option<crate::mcp::types::MCPContent> = None;
    if let Some(tool_calls) = &mut assistant_message.tool_calls {
        let mut break_index = None;
        let mut break_action = None;
        let mut ui_alias_enabled = true;
        let loop_threshold = circuit_breaker::load_loop_prevention_threshold().await;

        {
            let sessions = active_sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                ui_alias_enabled = circuit_breaker::is_builtin_alias_enabled(
                    session.metadata.agent_config.as_deref(),
                    "ui",
                );
                let messages = session.messages.read().await;
                let (call_name_by_id, call_signature_by_id) =
                    circuit_breaker::build_tool_call_indices(&messages);

                for (i, tool_call) in tool_calls.iter().enumerate() {
                    if let Some(action) = circuit_breaker::evaluate_circuit_breaker_action(
                        &messages,
                        tool_call,
                        &call_name_by_id,
                        &call_signature_by_id,
                        loop_threshold,
                    ) {
                        break_index = Some(i);
                        break_action = Some(action);
                        break;
                    }
                }
            }
        }

        if let Some(idx) = break_index {
            if let Some(action) = break_action {
                match action {
                    circuit_breaker::CircuitBreakerAction::HardBreak {
                        count,
                        tool_name,
                        args,
                    } => {
                        log::warn!(
                            "Circuit breaker triggered for session {} tool {} (count {})",
                            session_id,
                            tool_name,
                            count
                        );

                        if ui_alias_enabled {
                            let circuit_break_call = ToolCall {
                                id: uuid::Uuid::new_v4().to_string(),
                                function: ToolCallFunction {
                                    name: "ui__circuitBreak".to_string(),
                                    arguments: serde_json::json!({
                                        "toolName": tool_name,
                                        "repetitionCount": count,
                                        "args": args
                                    })
                                    .to_string(),
                                },
                                r#type: "function".to_string(),
                            };

                            tool_calls[idx] = circuit_break_call;
                            tool_calls.truncate(idx + 1);
                        } else {
                            log::warn!(
                                "UI alias disabled for session {}. Using text-only circuit break fallback.",
                                session_id
                            );

                            forced_circuit_break_message = Some(crate::mcp::types::MCPContent::Text {
                                text: format!(
                                    "⚠️ Circuit breaker triggered: detected runaway loop for tool '{}' (count {}).

The 'ui' builtin server is disabled for this session, so interactive circuit-break UI was skipped. Workflow was force-stopped to prevent further runaway calls.",
                                    tool_name, count
                                ),
                                is_error: None,
                            });
                        }
                    }
                    circuit_breaker::CircuitBreakerAction::NaturalRecoveryError {
                        count,
                        tool_name,
                        ..
                    } => {
                        log::warn!(
                            "Natural recovery (Error track) triggered for session {} tool {} (count {})",
                            session_id, tool_name, count
                        );
                        let entropy = uuid::Uuid::new_v4().to_string();
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_nanos() as usize;

                        let error_templates = [
                            "Wait, my action '{TOOL_NAME}' keeps failing and I am stuck in a loop. I must reflect on my previous state and consider a completely different alternative approach instead of repeating the identical action.",
                            "The tool '{TOOL_NAME}' has resulted in an error repeatedly. Let me stop using it and think about another way to achieve the goal.",
                            "Attempting '{TOOL_NAME}' with the same arguments is clearly not working. I should review the error messages carefully and change my strategy.",
                            "I'm caught in an error loop with '{TOOL_NAME}'. Let's halt this action. What am I missing in the configuration or arguments?",
                            "Calling '{TOOL_NAME}' again won't fix the issue. I need to formulate a new plan and avoid the path that leads to this failure.",
                            "I keep hitting the same wall with '{TOOL_NAME}'. Let me step back, analyze the root cause of this error, and try a different tool.",
                            "This repeated failure on '{TOOL_NAME}' indicates my approach is flawed. I must deviate from this pattern immediately and re-evaluate.",
                            "I must break this cycle. '{TOOL_NAME}' is consistently failing. I will stop executing it and instead focus on debugging the core problem.",
                            "There's no point in trying '{TOOL_NAME}' one more time here. I need to take a fundamentally different approach to this task.",
                            "I am stuck. The same error keeps popping up for '{TOOL_NAME}'. Let me pause, clear my assumptions, and look for an alternative method."
                        ];

                        let template = error_templates[nanos % error_templates.len()];
                        let recovery_thought = format!(
                            "{} [Entropy ID: {}]",
                            template.replace("{TOOL_NAME}", &tool_name),
                            entropy
                        );

                        let think_call = ToolCall {
                            id: uuid::Uuid::new_v4().to_string(),
                            function: ToolCallFunction {
                                name: "scratchpad__think".to_string(),
                                arguments: serde_json::json!({
                                    "thought": recovery_thought
                                })
                                .to_string(),
                            },
                            r#type: "function".to_string(),
                        };
                        tool_calls[idx] = think_call;
                        tool_calls.truncate(idx + 1);
                    }
                    circuit_breaker::CircuitBreakerAction::NaturalRecoverySuccess {
                        count,
                        tool_name,
                        ..
                    } => {
                        log::warn!(
                            "Natural recovery (Success track) triggered for session {} tool {} (count {})",
                            session_id, tool_name, count
                        );
                        let entropy = uuid::Uuid::new_v4().to_string();
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_nanos() as usize;

                        let success_templates = [
                            "I have repeatedly called '{TOOL_NAME}' successfully with identical parameters but I am not making progress. What was I originally scheduled to do? I need to focus on the next step immediately.",
                            "The repeated success of '{TOOL_NAME}' means the state has changed as intended, but I'm inexplicably repeating it. I must move forward to the next logical task.",
                            "Executing '{TOOL_NAME}' over and over with the same inputs is redundant. I have already achieved the result of this step. Time to proceed.",
                            "I'm looping on '{TOOL_NAME}' even though it's succeeding. I must break out of this repetition and execute the next action in my plan.",
                            "Why am I doing this? '{TOOL_NAME}' was already successful. Let me read my original plan and advance to the next unmet objective.",
                            "I need to advance. The repeated execution of '{TOOL_NAME}' is a loop. Let me stop this and focus on what remains to be done.",
                            "This is a redundant success loop. '{TOOL_NAME}' worked, so I should stop calling it and move on to the next phase of the workflow.",
                            "I've verified that '{TOOL_NAME}' succeeds. There's no need to rerun it. I will check my task list and transition to the subsequent step.",
                            "I am stuck in a pattern of repeating '{TOOL_NAME}'. I need to break this habit immediately and progress with the remainder of my objective.",
                            "Success on '{TOOL_NAME}' is verified. Repeating it adds no value. Let me pivot to the next action required to complete my goal."
                        ];

                        let template = success_templates[nanos % success_templates.len()];
                        let recovery_thought = format!(
                            "{} [Entropy ID: {}]",
                            template.replace("{TOOL_NAME}", &tool_name),
                            entropy
                        );

                        let think_call = ToolCall {
                            id: uuid::Uuid::new_v4().to_string(),
                            function: ToolCallFunction {
                                name: "scratchpad__think".to_string(),
                                arguments: serde_json::json!({
                                    "thought": recovery_thought
                                })
                                .to_string(),
                            },
                            r#type: "function".to_string(),
                        };
                        tool_calls[idx] = think_call;
                        tool_calls.truncate(idx + 1);
                    }
                }
            }
        }
    }

    if let Some(circuit_break_message) = forced_circuit_break_message {
        assistant_message.tool_calls = None;
        assistant_message.content = vec![circuit_break_message];
    }

    let post_response_compaction_pressure =
        calculate_post_response_compaction_pressure(&assistant_message).await;

    // 1. Add assistant message to cache
    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
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

    // 2. Emit MessageAdded event
    let message_added_event = crate::agent::events::AgentEvent::MessageAdded {
        session_id: session_id.clone(),
        message: Box::new(assistant_message.clone()),
    };
    crate::agent::tauri_events::emit_agent_event(app_handle, message_added_event)
        .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;

    // 3. Persist to DB asynchronously
    let msg_for_db = assistant_message.clone();

    tokio::spawn(async move {
        let repo = crate::state::get_message_repository();
        if let Err(e) = repo.insert(&msg_for_db).await {
            log::error!(
                "Failed to save assistant message to DB: msg_id={}, error={}",
                msg_for_db.id,
                e
            );
        }
    });

    // Parse tool calls
    // ⚡ Bolt: Use take() to move the vector out of the struct instead of cloning it, saving a deep copy.
    let tool_calls: Vec<ToolCall> = assistant_message.tool_calls.take().unwrap_or_default();

    if tool_calls.is_empty() {
        // Check if content is also empty (abnormal empty response).
        // Note: A message with tool calls but no content is VALID and normal.
        // We check that at least one content item has meaningful text (matching
        // the frontend's hasContent logic), so that Gemini-style empty-text
        // responses like [{type:"text", text:""}] are not treated as valid content.
        let has_content = assistant_message.content.iter().any(|c| match c {
            crate::mcp::types::MCPContent::Text { text, .. } => !text.trim().is_empty(),
            _ => true, // Non-text content (Image, Audio, Resource, etc.) is always meaningful
        });
        // ✅ FIX: Also check thinking field to allow thinking-only messages (Spec requirement)
        let has_thinking = assistant_message
            .thinking
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false);

        if !has_content && !has_thinking {
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
            return Ok(post_response_compaction_pressure.clone());
        }

        // ✅ NEW: Think-only message auto-recurring (Spec requirement 3)
        if has_thinking && !has_content {
            // Get current thinking_only_count
            let current_count = {
                let active = active_sessions.read().await;
                if let Some(session) = active.get(&session_id) {
                    *session.thinking_only_count.read().await
                } else {
                    0
                }
            };

            // Circuit breaker: max 3 consecutive thinking-only responses
            if current_count >= 3 {
                log::warn!(
                    "⚠️  Circuit breaker triggered for session {}: {} consecutive thinking-only responses. Forcing workflow completion.",
                    session_id, current_count
                );

                // Reset counter and complete workflow
                {
                    let active = active_sessions.write().await;
                    if let Some(session) = active.get(&session_id) {
                        *session.thinking_only_count.write().await = 0;
                    }
                }

                crate::agent::lifecycle::update_session_status(
                    session_repo,
                    active_sessions,
                    app_handle,
                    &session_id,
                    SessionStatus::Idle,
                )
                .await?;

                let attention_at = chrono::Utc::now().timestamp_millis();
                session_repo
                    .update_attention(
                        &session_id,
                        attention_at,
                        SessionAttentionReason::RecurringStop,
                    )
                    .await
                    .map_err(|e| format!("Failed to persist session attention: {}", e))?;

                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.clone(),
                    reason: crate::agent::events::WorkflowCompletionReason::RecurringStop,
                };
                crate::agent::tauri_events::emit_agent_event(app_handle, event)
                    .map_err(|e| format!("Failed to emit event: {}", e))?;

                log::info!(
                    "Workflow completed with circuit breaker for session: {}",
                    session_id
                );
                return Ok(post_response_compaction_pressure.clone());
            }

            // Increment thinking_only_count
            {
                let active = active_sessions.write().await;
                if let Some(session) = active.get(&session_id) {
                    let mut count = session.thinking_only_count.write().await;
                    *count += 1;
                    log::info!(
                        "🧠 Think-only message detected for session {} (attempt {}/3). Triggering next LLM turn (auto-recurring).",
                        session_id, *count
                    );
                }
            }

            match defer_for_post_response_compaction_if_needed(
                active_sessions,
                app_handle,
                &session_id,
                post_response_compaction_pressure.as_ref(),
                DeferredWorkflowStep::RequestCompletion,
            )
            .await
            {
                Ok(true) => {
                    log::info!(
                        "⏸️ Delaying follow-up LLM turn until post-response compaction finishes: session={}, assistant_message={}",
                        session_id,
                        assistant_message.id
                    );
                    return Ok(post_response_compaction_pressure.clone());
                }
                Ok(false) => {}
                Err(error) => {
                    log::warn!(
                        "⚠️ Failed to evaluate post-response compaction for session {} after assistant message {}: {}",
                        session_id,
                        assistant_message.id,
                        error
                    );
                }
            }

            // Auto-recurring: trigger next LLM turn
            return request_llm_completion(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id,
            )
            .await
            .map(|_| post_response_compaction_pressure.clone())
            .map_err(String::from);
        }

        // ✅ Content present: reset thinking_only_count
        {
            let active = active_sessions.write().await;
            if let Some(session) = active.get(&session_id) {
                *session.thinking_only_count.write().await = 0;
            }
        }

        // Check for pending messages before finishing
        let has_pending = {
            let active = active_sessions.read().await;
            if let Some(session) = active.get(&session_id) {
                session.pending_events.read().await.count() > 0
            } else {
                false
            }
        };

        if has_pending {
            match defer_for_post_response_compaction_if_needed(
                active_sessions,
                app_handle,
                &session_id,
                post_response_compaction_pressure.as_ref(),
                DeferredWorkflowStep::RequestCompletion,
            )
            .await
            {
                Ok(true) => {
                    log::info!(
                        "⏸️ Delaying pending-message continuation until post-response compaction finishes: session={}, assistant_message={}",
                        session_id,
                        assistant_message.id
                    );
                    return Ok(post_response_compaction_pressure.clone());
                }
                Ok(false) => {}
                Err(error) => {
                    log::warn!(
                        "⚠️ Failed to evaluate post-response compaction for session {} after assistant message {}: {}",
                        session_id,
                        assistant_message.id,
                        error
                    );
                }
            }

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
            .map(|_| post_response_compaction_pressure.clone())
            .map_err(String::from);
        }

        match defer_for_post_response_compaction_if_needed(
            active_sessions,
            app_handle,
            &session_id,
            post_response_compaction_pressure.as_ref(),
            DeferredWorkflowStep::FinalizeWorkflow {
                reason: crate::agent::events::WorkflowCompletionReason::Natural,
            },
        )
        .await
        {
            Ok(true) => {
                log::info!(
                    "⏸️ Delaying workflow completion until post-response compaction finishes: session={}, assistant_message={}",
                    session_id,
                    assistant_message.id
                );
                return Ok(post_response_compaction_pressure.clone());
            }
            Ok(false) => {}
            Err(error) => {
                log::warn!(
                    "⚠️ Failed to evaluate post-response compaction for session {} after assistant message {}: {}",
                    session_id,
                    assistant_message.id,
                    error
                );
            }
        }

        // No pending messages and no blocking post-response compaction, finish workflow
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
        // Tools found! Initiate execution
        log::info!(
            "Processing {} tool calls for session: {}",
            tool_calls.len(),
            session_id
        );

        // Reset thinking_only_count (tool calls = normal workflow progress)
        {
            let active = active_sessions.write().await;
            if let Some(session) = active.get(&session_id) {
                *session.thinking_only_count.write().await = 0;
            }
        }

        match defer_for_post_response_compaction_if_needed(
            active_sessions,
            app_handle,
            &session_id,
            post_response_compaction_pressure.as_ref(),
            DeferredWorkflowStep::ExecuteToolCalls {
                assistant_message_id: assistant_message.id.clone(),
                tool_calls: tool_calls.clone(),
            },
        )
        .await
        {
            Ok(true) => {
                log::info!(
                    "⏸️ Delaying tool execution until post-response compaction finishes: session={}, assistant_message={}, tool_calls={}",
                    session_id,
                    assistant_message.id,
                    tool_calls.len()
                );
                return Ok(post_response_compaction_pressure.clone());
            }
            Ok(false) => {}
            Err(error) => {
                log::warn!(
                    "⚠️ Failed to evaluate post-response compaction before tool execution for session {} after assistant message {}: {}",
                    session_id,
                    assistant_message.id,
                    error
                );
            }
        }

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
        {
            let mut active = active_sessions.write().await;
            if let Some(session) = active.get_mut(&session_id) {
                let expected_tool_call_ids: std::collections::HashSet<String> =
                    tool_calls.iter().map(|tc| tc.id.clone()).collect();
                session.pending_execution = Some(crate::agent::state::PendingToolExecution {
                    message_id: assistant_message.id.clone(),
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

    Ok(post_response_compaction_pressure)
}

/// Handle LLM error from frontend
pub async fn handle_llm_error(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    error: AgentRuntimeError,
) -> Result<(), String> {
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
                return Ok(());
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
    .await
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
