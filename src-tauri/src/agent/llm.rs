use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::commands::messages_commands::Message;
use crate::mcp::service_proxy::MCPServiceProxy;
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

/// Request LLM completion from frontend
pub async fn request_llm_completion(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    // Read messages from in-memory cache
    let messages = {
        let sessions = active_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let messages_lock = session.messages.read().await;
        messages_lock.clone()
    };

    log::info!(
        "🔄 Message stack for LLM request: session={}, count={}, first_msg_id={}, last_msg_id={}",
        session_id,
        messages.len(),
        messages.first().map(|m| m.id.as_str()).unwrap_or("none"),
        messages.last().map(|m| m.id.as_str()).unwrap_or("none")
    );

    // Get agent config
    let active = active_sessions.read().await;
    let session = active
        .get(&session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let agent_config = session
        .metadata
        .agent_config
        .as_ref()
        .ok_or_else(|| "Agent configuration is required but not found".to_string())
        .and_then(|json| crate::agent::AgentConfig::from_json(json).map_err(|e| e.to_string()))?;

    let agent_config_clone = agent_config.clone();
    let model = agent_config.model;
    let provider = agent_config.provider;
    let temperature = Some(agent_config.temperature);
    let max_tokens = agent_config.max_tokens;

    drop(active);

    // Build system prompt
    let system_prompt =
        Some(build_session_system_prompt(active_sessions, proxy_manager, &session_id).await?);

    // Collect available tools
    let available_tools = crate::agent::tools::collect_available_tools(
        &session_id,
        &agent_config_clone,
        proxy_manager,
    )
    .await
    .ok();

    // Emit event
    #[derive(Clone, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CompletionRequest {
        session_id: String,
        messages: Vec<Message>,
        model: String,
        provider: String,
        system_prompt: Option<String>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        available_tools: Option<Vec<crate::mcp::types::MCPTool>>,
    }

    let request = CompletionRequest {
        session_id: session_id.clone(),
        messages,
        model,
        provider,
        system_prompt,
        temperature,
        max_tokens,
        available_tools,
    };

    app_handle
        .emit("llm:completion-request", request)
        .map_err(|e| format!("Failed to emit LLM completion request: {}", e))?;

    log::info!("Emitted LLM completion request for session: {}", session_id);

    Ok(())
}

/// Handle an LLM response from the frontend
pub async fn handle_llm_response(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    mut assistant_message: Message,
) -> Result<(), String> {
    // Check cancellation
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            if session.cancellation_token.is_cancelled() {
                log::info!("Workflow cancelled for session: {}", session_id);
                return Err("Workflow was cancelled".to_string());
            }
        }
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

                    // Threshold: 3 (0-based count of previous occurrences: 0, 1 -> 2 means 3rd occurrence)
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
        // Workflow completed
        crate::agent::lifecycle::update_session_status(
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

        // Initialize pending execution state
        {
            let mut active = active_sessions.write().await;
            if let Some(session) = active.get_mut(&session_id) {
                session.pending_execution = Some(crate::agent::state::PendingToolExecution {
                    total_expected: tool_calls.len(),
                    results: Vec::new(),
                    tool_names: tool_calls
                        .iter()
                        .map(|tc| (tc.id.clone(), tc.function.name.clone()))
                        .collect(),
                });
            }
        }

        // Execute tool calls
        for tool_call in tool_calls {
            let tool_name = tool_call.function.name.clone();

            let effective_tool_name = tool_name.clone();
            let effective_tool_call = tool_call.clone();

            // Emit ToolExecutionStarted
            let event = crate::agent::events::AgentEvent::ToolExecutionStarted {
                session_id: session_id.clone(),
                tool_name: effective_tool_name.clone(),
            };
            crate::agent::events::emit_agent_event(app_handle, event)
                .map_err(|e| format!("Failed to emit tool execution started event: {}", e))?;

            // Spawn async task for execution
            let active_sessions_clone = active_sessions.clone();
            let app_handle_clone = app_handle.clone();
            let proxy_manager_clone = proxy_manager.clone();
            let session_id_clone = session_id.clone();
            let tool_call_clone = effective_tool_call;
            let tool_name_owned = effective_tool_name;

            tokio::spawn(async move {
                let tool_call_id = tool_call_clone.id;
                let args_str = tool_call_clone.function.arguments;

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
                        handle_tool_result_and_continue(
                            &active_sessions_clone,
                            &proxy_manager_clone,
                            &app_handle_clone,
                            session_id_clone,
                            tool_call_id,
                            result,
                        )
                        .await;
                        return;
                    }
                };

                // Call tool
                let result = match proxy_manager_clone
                    .call_tool(&session_id_clone, &tool_name_owned, args)
                    .await
                {
                    Ok(response) => {
                        let content = response
                            .result
                            .as_ref()
                            .and_then(|r| serde_json::to_string_pretty(r).ok())
                            .unwrap_or_else(|| "{}".to_string());

                        let is_error = response.error.is_some();
                        let error_msg = response.error.map(|e| e.message);

                        crate::commands::agent_commands::ToolExecutionResult {
                            success: !is_error,
                            content,
                            error: error_msg,
                            is_error,
                            mcp_content: crate::agent::tools::convert_mcp_response_content(
                                response.result,
                            ),
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
                handle_tool_result_and_continue(
                    &active_sessions_clone,
                    &proxy_manager_clone,
                    &app_handle_clone,
                    session_id_clone,
                    tool_call_id,
                    result,
                )
                .await;
            });
        }
    }

    Ok(())
}

/// Helper to handle tool result and trigger next steps if valid
async fn handle_tool_result_and_continue(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    tool_call_id: String,
    result: crate::commands::agent_commands::ToolExecutionResult,
) {
    match crate::agent::tools::handle_tool_result(
        active_sessions,
        app_handle,
        session_id.clone(),
        tool_call_id,
        result,
    )
    .await
    {
        Ok(Some(accumulated_messages)) => {
            log::info!(
                "All tool results received for session {}. Proceeding.",
                session_id
            );

            // Add to cache
            {
                let sessions = active_sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    let mut messages = session.messages.write().await;
                    for msg in &accumulated_messages {
                        messages.push(msg.clone());
                        if messages.len() > MAX_CACHED_MESSAGES {
                            messages.remove(0);
                        }
                    }
                }
            }

            // Emit MessageAdded for each
            for msg in &accumulated_messages {
                let event = crate::agent::events::AgentEvent::MessageAdded {
                    session_id: session_id.clone(),
                    message: Box::new(msg.clone()),
                };
                let _ = crate::agent::events::emit_agent_event(app_handle, event);
            }

            // Persist to DB
            let msgs_for_db = accumulated_messages.clone();

            tokio::spawn(async move {
                let repo = crate::state::get_message_repository();
                for msg in msgs_for_db {
                    let _ = repo.insert(&msg).await;
                }
            });

            // Check for UI interaction (stop condition)
            let has_ui_interaction = accumulated_messages.iter().any(|msg| {
                msg.content
                    .iter()
                    .any(|c| matches!(c, MCPContent::Resource { .. }))
            });

            if has_ui_interaction {
                log::info!(
                    "UI interaction detected for session {}. Stopping loop.",
                    session_id
                );
                let _ = crate::agent::lifecycle::update_session_status(
                    active_sessions,
                    app_handle,
                    &session_id,
                    SessionStatus::Idle,
                )
                .await;
                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.clone(),
                };
                let _ = crate::agent::events::emit_agent_event(app_handle, event);
            } else {
                // Request next LLM completion
                if let Err(e) =
                    request_llm_completion(active_sessions, proxy_manager, app_handle, session_id)
                        .await
                {
                    log::error!("Failed to request LLM completion: {}", e);
                }
            }
        }
        Ok(Option::None) => {
            // Still waiting for other tools
        }
        Err(e) => {
            log::error!("Error handling tool result: {}", e);
        }
    }
}

/// Handle LLM error from frontend
pub async fn handle_llm_error(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    error: String,
) -> Result<(), String> {
    log::error!("LLM error for session {}: {}", session_id, error);

    crate::agent::lifecycle::update_session_status(
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Idle,
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

/// Build complete system prompt for session (wrapper)
async fn build_session_system_prompt(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    session_id: &str,
) -> Result<String, String> {
    let active = active_sessions.read().await;
    let session = active
        .get(session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let agent_config = session
        .metadata
        .agent_config
        .as_ref()
        .ok_or_else(|| "Agent configuration is required but not found".to_string())
        .and_then(|json| crate::agent::AgentConfig::from_json(json).map_err(|e| e.to_string()))?;

    let config_clone = agent_config.clone();
    drop(active);

    let proxy = proxy_manager.get_proxy(session_id).await;

    build_system_prompt(&config_clone, proxy).await
}

/// Build complete system prompt (Pure logic)
///
/// Structure (Legacy-inspired, tool-first approach):
/// 1. Agent Identity & Strategy (who am I, how do I work)
/// 2. Service Contexts (tools & current state - immediately actionable)
/// 3. Time & Location (contextual reference information)
pub async fn build_system_prompt(
    agent_config: &crate::agent::AgentConfig,
    proxy: Option<Arc<MCPServiceProxy>>,
) -> Result<String, String> {
    let mut parts = Vec::new();

    // 1. Agent Identity & Strategy (first priority)
    if !agent_config.system_prompt.trim().is_empty() {
        parts.push(agent_config.system_prompt.clone());
    }

    // 2. Service Contexts - immediately actionable information (second priority)
    if let Some(p) = proxy {
        let contexts = p.get_service_contexts().await;

        if !contexts.is_empty() {
            parts.push("\n\n## Available Tools & Current State\n".to_string());

            for (_tool_id, service_context) in contexts {
                if !service_context.context_prompt.trim().is_empty() {
                    parts.push(service_context.context_prompt);
                }
            }
        }
    }

    // 3. Time and location context (reference information, last)
    parts.push(build_time_location_context());

    Ok(parts.join("\n"))
}

/// Build time and location context for system prompt
fn build_time_location_context() -> String {
    use chrono::{Datelike, Local, Timelike};

    let now = Local::now();

    // Format date as "Monday, December 30, 2025"
    let weekday = match now.weekday() {
        chrono::Weekday::Mon => "Monday",
        chrono::Weekday::Tue => "Tuesday",
        chrono::Weekday::Wed => "Wednesday",
        chrono::Weekday::Thu => "Thursday",
        chrono::Weekday::Fri => "Friday",
        chrono::Weekday::Sat => "Saturday",
        chrono::Weekday::Sun => "Sunday",
    };

    let month = match now.month() {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    };

    let current_date = format!("{}, {} {}, {}", weekday, month, now.day(), now.year());

    // Format time with timezone
    let current_time = format!(
        "{:02}:{:02}:{:02} {}",
        now.hour(),
        now.minute(),
        now.second(),
        now.offset()
    );

    // Get timezone name
    let timezone = format!("{}", now.offset());

    format!(
        "# Current Context Information\n\n\
        ## Date and Time\n\
        - **Current Date**: {}\n\
        - **Current Time**: {}\n\
        - **Timezone**: {}\n\n\
        *This information is automatically updated to help you understand the user's current temporal context.*",
        current_date,
        current_time,
        timezone
    )
}
