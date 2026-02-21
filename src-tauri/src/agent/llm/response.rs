use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::completion::request_llm_completion;

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
    if let Some(tool_calls) = &mut assistant_message.tool_calls {
        let mut break_index = None;
        let mut break_info = None;

        {
            let sessions = active_sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                let messages = session.messages.read().await;

                for (i, tool_call) in tool_calls.iter().enumerate() {
                    let tool_name = &tool_call.function.name;
                    // Skip if it's already a circuit break call
                    if tool_name == "builtin_ui__circuitBreak" {
                        continue;
                    }

                    let args = &tool_call.function.arguments;
                    let current_signature = format!("{}:{}", tool_name, args);

                    let mut repetition_count = 0;
                    // 1. Count in history (consecutive messages containing the tool)
                    for msg in messages.iter().rev() {
                        if let Some(msg_tool_calls) = &msg.tool_calls {
                            let mut found_in_msg = false;
                            for tc in msg_tool_calls {
                                let sig = format!("{}:{}", tc.function.name, tc.function.arguments);
                                if sig == current_signature {
                                    repetition_count += 1;
                                    found_in_msg = true;
                                    break;
                                }
                            }

                            if !found_in_msg && msg.role != "tool" {
                                break;
                            }
                        } else if msg.role != "tool" {
                            break;
                        }
                    }

                    // 2. Count in current batch (calls before this one)
                    let batch_count = tool_calls[0..i]
                        .iter()
                        .filter(|tc| {
                            let sig = format!("{}:{}", tc.function.name, tc.function.arguments);
                            sig == current_signature
                        })
                        .count();

                    let total_count = repetition_count + batch_count;

                    // Threshold: Trigger on 3rd occurrence (total_count >= 2 means 2 previous + 1 current = 3 total)
                    if total_count >= 2 {
                        break_index = Some(i);
                        break_info = Some((tool_name.clone(), total_count + 1, args.clone()));
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

                let circuit_break_call = ToolCall {
                    id: uuid::Uuid::new_v4().to_string(),
                    function: ToolCallFunction {
                        name: "builtin_ui__circuitBreak".to_string(),
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
            }
        }
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
    let tool_calls: Vec<ToolCall> = if let Some(tool_calls_vec) = &assistant_message.tool_calls {
        tool_calls_vec.clone()
    } else {
        Vec::new()
    };

    if tool_calls.is_empty() {
        // Check if content is also empty (abnormal empty response)
        // Note: A message with tool calls but no content is VALID and normal
        let has_content = !assistant_message.content.is_empty();
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
                error: "EMPTY_LLM_RESPONSE: The AI model returned an empty response with no content, tool calls, or thinking. This may indicate a model inference issue, context overflow, or generation failure. Please try again.".to_string(),
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

        // No pending messages, finish workflow
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
        let tool_calls_clone = tool_calls.clone();

        tokio::spawn(async move {
            for tool_call in tool_calls_clone {
                let tool_name = tool_call.function.name.clone();
                let tool_call_id = tool_call.id.clone();
                let args_str = tool_call.function.arguments.clone();

                // Emit ToolExecutionStarted
                let event = crate::agent::events::AgentEvent::ToolExecutionStarted {
                    session_id: session_id_clone.clone(),
                    tool_name: tool_name.clone(),
                };
                if let Err(e) = crate::agent::events::emit_agent_event(&app_handle_clone, event) {
                    log::error!("Failed to emit tool execution started event: {}", e);
                }

                // Parse arguments
                let args = match serde_json::from_str::<serde_json::Value>(&args_str) {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("Failed to parse tool arguments: {}", e);
                        let result = crate::commands::agent_commands::ToolExecutionResult {
                            success: false,
                            content: String::new(),
                            error: Some(format!("Failed to parse args: {}", e)),
                            is_error: true,
                            mcp_content: None,
                        };
                        // Handle result (error case)
                        if let Err(e) = crate::agent::workflow::continue_workflow_after_tool(
                            &session_repo_clone,
                            &active_sessions_clone,
                            &proxy_manager_clone,
                            &app_handle_clone,
                            session_id_clone.clone(),
                            tool_call_id,
                            result,
                        )
                        .await
                        {
                            log::error!("Error continuing workflow after failed tool parse: {}", e);
                        }
                        continue; // Proceed to next tool
                    }
                };

                // Call tool
                let result = match proxy_manager_clone
                    .call_tool(&session_id_clone, &tool_name, args)
                    .await
                {
                    Ok(response) => {
                        let mcp_content = crate::agent::tools::convert_mcp_response_content(
                            response.result.clone(),
                        );

                        // For logging/debugging only (not used in tool messages)
                        let debug_content = response
                            .result
                            .as_ref()
                            .and_then(|r| serde_json::to_string_pretty(r).ok())
                            .unwrap_or_else(|| "{}".to_string());

                        let is_error = response.error.is_some();
                        let error_msg = response.error.map(|e| e.message);

                        crate::commands::agent_commands::ToolExecutionResult {
                            success: !is_error,
                            content: debug_content,
                            error: error_msg,
                            is_error,
                            mcp_content,
                        }
                    }
                    Err(e) => crate::commands::agent_commands::ToolExecutionResult {
                        success: false,
                        content: String::new(),
                        error: Some(e),
                        is_error: true,
                        mcp_content: None,
                    },
                };

                // Handle result and potentially continue workflow
                if let Err(e) = crate::agent::workflow::continue_workflow_after_tool(
                    &session_repo_clone,
                    &active_sessions_clone,
                    &proxy_manager_clone,
                    &app_handle_clone,
                    session_id_clone.clone(),
                    tool_call_id,
                    result,
                )
                .await
                {
                    log::error!("Error continuing workflow after tool execution: {}", e);
                }
            }
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
