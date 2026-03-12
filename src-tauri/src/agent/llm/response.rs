use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::{SessionRepository, SessionStatus};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::circuit_breaker;
use super::completion::request_llm_completion;
use super::tool_execution;

fn has_nonempty_content(content: &[MCPContent]) -> bool {
    content.iter().any(|item| match item {
        MCPContent::Text { text, .. } => !text.trim().is_empty(),
        _ => true,
    })
}

fn has_completion_usage(usage: Option<&Value>) -> bool {
    fn parse_positive_number(value: &Value) -> Option<bool> {
        match value {
            Value::Number(number) => number
                .as_u64()
                .map(|count| count > 0)
                .or_else(|| number.as_i64().map(|count| count > 0))
                .or_else(|| number.as_f64().map(|count| count > 0.0)),
            Value::String(text) => text.parse::<f64>().ok().map(|count| count > 0.0),
            _ => None,
        }
    }

    usage
        .and_then(|value| value.as_object())
        .and_then(|usage_map| {
            usage_map
                .get("completionTokens")
                .or_else(|| usage_map.get("completion_tokens"))
        })
        .and_then(parse_positive_number)
        .unwrap_or(false)
}

pub fn is_effectively_empty_llm_response(message: &Message) -> bool {
    let has_tool_calls = message
        .tool_calls
        .as_ref()
        .map(|calls| !calls.is_empty())
        .unwrap_or(false);
    let has_thinking = message
        .thinking
        .as_ref()
        .map(|thinking| !thinking.is_empty())
        .unwrap_or(false);

    !has_tool_calls
        && !has_nonempty_content(&message.content)
        && !has_thinking
        && !has_completion_usage(message.usage.as_ref())
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
        crate::agent::events::emit_agent_event(app_handle, event)
            .map_err(|e| format!("Failed to emit WorkflowStarted event: {}", e))?;
    }

    // [Circuit Breaker] Pre-process: Check for loops and inject circuit breaker if needed
    let mut forced_circuit_break_message: Option<crate::mcp::types::MCPContent> = None;
    if let Some(tool_calls) = &mut assistant_message.tool_calls {
        let mut break_index = None;
        let mut break_info = None;
        let mut ui_alias_enabled = true;

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
                    let tool_name = &tool_call.function.name;
                    let args = &tool_call.function.arguments;
                    if let Some(trigger_count) = circuit_breaker::evaluate_circuit_breaker_count(
                        &messages,
                        tool_call,
                        &call_name_by_id,
                        &call_signature_by_id,
                    ) {
                        break_index = Some(i);
                        break_info = Some((tool_name.clone(), trigger_count, args.clone()));
                        break;
                    }
                }
            }
        }

        if let Some(idx) = break_index {
            if let Some((name, count, args)) = break_info {
                log::warn!(
                    "Circuit breaker triggered for session {} tool {} (count {})",
                    session_id,
                    name,
                    count
                );

                if ui_alias_enabled {
                    let circuit_break_call = ToolCall {
                        id: uuid::Uuid::new_v4().to_string(),
                        function: ToolCallFunction {
                            name: "ui__circuitBreak".to_string(),
                            arguments: serde_json::json!({
                                "toolName": name,
                                "repetitionCount": count,
                                "args": args
                            })
                            .to_string(),
                        },
                        r#type: "function".to_string(),
                    };

                    // Replace the triggering tool call and remove subsequent ones
                    tool_calls[idx] = circuit_break_call;
                    tool_calls.truncate(idx + 1);
                } else {
                    log::warn!(
                        "UI alias disabled for session {}. Using text-only circuit break fallback.",
                        session_id
                    );

                    forced_circuit_break_message = Some(crate::mcp::types::MCPContent::Text {
                        text: format!(
                            "⚠️ Circuit breaker triggered: detected runaway loop for tool '{}' (count {}).\n\nThe 'ui' builtin server is disabled for this session, so interactive circuit-break UI was skipped. Workflow was force-stopped to prevent further runaway calls.",
                            name, count
                        ),
                        is_error: None,
                    });
                }
            }
        }
    }

    if let Some(circuit_break_message) = forced_circuit_break_message {
        assistant_message.tool_calls = None;
        assistant_message.content = vec![circuit_break_message];
    }

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
    crate::agent::events::emit_agent_event(app_handle, message_added_event)
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
        let has_content = has_nonempty_content(&assistant_message.content);
        let has_thinking = assistant_message
            .thinking
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false);
        let has_completion_usage = has_completion_usage(assistant_message.usage.as_ref());

        if !has_content && !has_thinking && !has_completion_usage {
            log::warn!(
                "⚠️  Empty LLM response detected for session {}: no content, tool calls, thinking, or completion usage. This may indicate a model inference issue.",
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
                error: "EMPTY_LLM_RESPONSE: The AI model returned an empty response with no content, tool calls, thinking, or completion usage. This may indicate a model inference issue, context overflow, or generation failure. Please try again.".to_string(),
            };
            crate::agent::events::emit_agent_event(app_handle, error_event)
                .map_err(|e| format!("Failed to emit WorkflowError event: {}", e))?;
            return Ok(());
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

                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.clone(),
                };
                crate::agent::events::emit_agent_event(app_handle, event)
                    .map_err(|e| format!("Failed to emit event: {}", e))?;

                log::info!(
                    "Workflow completed with circuit breaker for session: {}",
                    session_id
                );
                return Ok(());
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

            // Auto-recurring: trigger next LLM turn
            return request_llm_completion(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id,
            )
            .await;
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
            .await;
        }

        // A text-only response without a UI Resource means the agent has not
        // presented a result to the user yet. Recur to prompt it to do so.
        let last_tool_has_ui_resource = {
            let active = active_sessions.read().await;
            if let Some(session) = active.get(&session_id) {
                let messages = session.messages.read().await;
                messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "tool")
                    .map(|m| {
                        m.content.iter().any(|c| matches!(c, MCPContent::Resource { .. }))
                    })
                    .unwrap_or(false)
            } else {
                false
            }
        };

        if !last_tool_has_ui_resource {
            // Circuit breaker: guard against infinite "please present" loops
            let current_count = {
                let active = active_sessions.read().await;
                if let Some(session) = active.get(&session_id) {
                    *session.text_only_no_ui_count.read().await
                } else {
                    0
                }
            };

            if current_count >= 3 {
                log::warn!(
                    "⚠️  Circuit breaker triggered for session {}: {} consecutive text-only responses without UI Resource. Forcing workflow completion.",
                    session_id, current_count
                );
                let active = active_sessions.write().await;
                if let Some(session) = active.get(&session_id) {
                    *session.text_only_no_ui_count.write().await = 0;
                }
            } else {
                {
                    let active = active_sessions.write().await;
                    if let Some(session) = active.get(&session_id) {
                        let mut count = session.text_only_no_ui_count.write().await;
                        *count += 1;
                        log::info!(
                            "🔄 Text-only response with no UI Resource for session {} (attempt {}/3). Recurring to prompt presentation.",
                            session_id, *count
                        );
                    }
                }
                return request_llm_completion(
                    session_repo,
                    active_sessions,
                    proxy_manager,
                    app_handle,
                    session_id,
                )
                .await;
            }
        }

        // Reset text_only_no_ui_count: either UI Resource found or circuit breaker hit
        {
            let active = active_sessions.write().await;
            if let Some(session) = active.get(&session_id) {
                *session.text_only_no_ui_count.write().await = 0;
            }
        }

        // Last tool result contains a UI Resource (or circuit breaker) — finish workflow.
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
        };
        crate::agent::events::emit_agent_event(app_handle, event)
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
                *session.text_only_no_ui_count.write().await = 0;
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
        // (e.g., writeFile followed by replaceLines on the same file)
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
pub async fn handle_llm_error(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    error: String,
) -> Result<(), String> {
    log::error!("LLM error for session {}: {}", session_id, error);

    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Error,
    )
    .await?;

    let event = crate::agent::events::AgentEvent::WorkflowError {
        session_id: session_id.clone(),
        error: error.clone(),
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit error event: {}", e))?;

    Ok(())
}
