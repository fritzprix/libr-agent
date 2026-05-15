use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

use crate::agent::AgentSessionManager;
use crate::mcp::builtin::error_guidance::ErrorCategory;
use crate::mcp::types::MCPResult;
use crate::models::chat::MessageSource;
use crate::repositories::assistant_repository::AssistantRepository;
use crate::repositories::session_repository::SessionRepository;

use super::cache::{min_interval_notice, unchanged_messages_notice};
use super::formatting::*;
use super::types::*;
use super::utils::*;

/// Helper to map raw session JSON to the standardized AgentSessionResponse.
fn map_session_response(
    session: &Value,
    turn_count: usize,
    latest_result: Option<String>,
) -> AgentSessionResponse {
    AgentSessionResponse {
        id: session
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        name: session
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed")
            .to_string(),
        status: extract_session_status(session),
        assistant_id: extract_assistant_id_from_session_value(session)
            .unwrap_or_else(|| "unknown".to_string()),
        turn_count,
        latest_result,
    }
}

pub async fn handle_tool_call(
    tool_name: &str,
    args: Value,
    caller_session_id: Option<String>,
    manager: Option<&AgentSessionManager>,
) -> Result<MCPResult, String> {
    match tool_name {
        "healthCheck" => {
            let data = json!({
                "status": "ok",
                "service": "session_api_direct",
                "transport": "in_process",
                "managerAvailable": manager.is_some(),
            });
            Ok(success_result(
                "Session API direct backend health check succeeded.".to_string(),
                data,
            ))
        }
        "spawnAgent" => {
            let manager = manager.ok_or_else(|| {
                "AgentSessionManager not available for legacy session API tools".to_string()
            })?;
            let assistant_id = read_required_string(&args, "assistantId")?;
            let request = read_required_string(&args, "request")?;

            let parent_session_id = caller_session_id
                .clone()
                .ok_or_else(|| "spawnAgent requires a caller session context".to_string())?;

            let mut body = json!({
                "parentSessionId": parent_session_id,
                "assistantId": assistant_id,
                "request": request,
            });

            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = Value::String(name.to_string());
            }

            if let Some(path) = args.get("workspacePath").and_then(|v| v.as_str()) {
                body["workspacePath"] = Value::String(path.to_string());
            }

            if let Some(model) = args.get("model").and_then(|v| v.as_str()) {
                body["model"] = Value::String(model.to_string());
            }

            if let Some(provider) = args.get("provider").and_then(|v| v.as_str()) {
                body["provider"] = Value::String(provider.to_string());
            }

            let await_completion = args
                .get("awaitCompletion")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let timeout_seconds = args
                .get("timeoutSeconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(180);

            let request: crate::agent::types::CreateSessionRequest =
                serde_json::from_value(body)
                    .map_err(|e| format!("Invalid spawnAgent arguments: {}", e))?;
            let caller_session_id = caller_session_id
                .clone()
                .ok_or_else(|| "spawnAgent requires a caller session context".to_string())?;

            let response = crate::services::AgentService::spawn_agent_with_source(
                manager,
                request,
                Some(MessageSource::SwarmLegacy),
            )
            .await
            .map_err(|e| {
                if e.contains("Assistant not found:") {
                    return format!(
                        "Assistant '{}' not found. Use listAgentTypes to see available types.",
                        assistant_id
                    );
                }
                e
            })?;

            let child_id = response.id.as_str();
            let child_id_owned = child_id.to_string();
            let session_data = fetch_session_value(manager, &child_id_owned)
                .await?
                .ok_or_else(|| format!("Spawned session '{}' not found", child_id_owned))?;

            if !await_completion {
                return Ok(success_result(
                    format!(
                        "Child agent '{}' spawned successfully (ID: {}).\n\nUse awaitAgent(\"{}\") to wait for completion and fetch results.",
                        session_data
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unnamed"),
                        child_id_owned, child_id_owned
                    ),
                    SwarmOperationResponse {
                        operation: "spawn".to_string(),
                        session: map_session_response(&session_data, 0, None),
                        messages_count: None,
                    },
                ));
            }

            // awaitCompletion=true: SP2 two-phase transition + SP1 push-notify wait.
            let wait_result = {
                let gate = crate::state::get_concurrency_gate();
                let mut active_permit = Some(
                    manager
                        .take_active_session_permit(&caller_session_id)
                        .await
                        .ok_or_else(|| {
                            format!(
                                "Caller session {} is not holding an active concurrency permit",
                                caller_session_id
                            )
                        })?,
                );
                let suspended = match gate.suspend_agent(&mut active_permit).await {
                    Ok(suspended) => suspended,
                    Err(error) => {
                        if let Some(permit) = active_permit.take() {
                            manager
                                .restore_active_session_permit(&caller_session_id, permit)
                                .await?;
                        }
                        return Err(error);
                    }
                };
                let res = wait_until_session_terminal(
                    manager,
                    &child_id_owned,
                    timeout_seconds,
                    Some(&caller_session_id),
                )
                .await;
                let resumed = suspended.resume().await?;
                manager
                    .restore_active_session_permit(&caller_session_id, resumed)
                    .await?;
                res
            };

            let (session_data, poll_count) = match handle_wait_timeout_result(
                wait_result,
                Some(manager),
                &child_id_owned,
                timeout_seconds,
                "spawnAgent",
                true,
            )
            .await
            {
                Ok(res) => res,
                Err(mcp_res) => return mcp_res,
            };

            let final_status = extract_session_status(&session_data);
            let turn_count = count_session_turns(&child_id_owned).await;

            let messages_data = fetch_messages_value(&child_id_owned, 20).await?;

            let messages = messages_data
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let (message_id, assistant_text) = latest_assistant_message_text(&messages, None)
                .unwrap_or((
                    "none".to_string(),
                    "No assistant text message found.".to_string(),
                ));

            let text = format!(
                "Child session {} completed with status '{}' after {} polls ({} turns).\n\nLatest assistant result [{}]:\n{}",
                child_id_owned, final_status, poll_count, turn_count, message_id, assistant_text
            );

            Ok(success_result(
                text,
                SwarmOperationResponse {
                    operation: "spawn_and_await".to_string(),
                    session: map_session_response(&session_data, turn_count, Some(assistant_text)),
                    messages_count: Some(messages.len()),
                },
            ))
        }
        "getAgentStatus" => {
            let manager = manager.ok_or_else(|| {
                "AgentSessionManager not available for legacy session API tools".to_string()
            })?;
            let session_id = read_required_string(&args, "sessionId")?;
            let data = match fetch_session_value(manager, &session_id).await? {
                Some(data) => data,
                None => return Ok(session_not_found_error("Get Agent Status", &session_id)),
            };

            let turn_count = count_session_turns(&session_id).await;
            let status = extract_session_status(&data);
            let name = data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed");

            Ok(success_result(
                format!("Session {} ({}): status={}", name, session_id, status),
                map_session_response(&data, turn_count, None),
            ))
        }
        "awaitAgent" => {
            let manager = manager.ok_or_else(|| {
                "AgentSessionManager not available for legacy session API tools".to_string()
            })?;
            let session_id = read_required_string(&args, "sessionId")?;
            let caller_session_id = caller_session_id
                .clone()
                .ok_or_else(|| "awaitAgent requires a caller session context".to_string())?;

            let timeout_seconds = args
                .get("timeoutSeconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(180);
            let result_message_limit = args
                .get("resultMessageLimit")
                .and_then(|v| v.as_u64())
                .map(|v| v.clamp(1, 200))
                .unwrap_or(20);
            let assistant_message_max_chars = args
                .get("assistantMessageMaxChars")
                .and_then(|v| v.as_u64())
                .map(|v| v.min(200000) as usize)
                .filter(|v| *v > 0);

            // Pre-check session existence and state
            let initial_session = match fetch_session_value(manager, &session_id).await? {
                Some(data) => data,
                None => return Ok(session_not_found_error("Await Agent", &session_id)),
            };

            if extract_session_status(&initial_session) == "paused" {
                // ... (auto-resume logic remains same but structured)
                let msgs_data = fetch_messages_value(&session_id, 5)
                    .await
                    .unwrap_or_default();
                let msgs = msgs_data
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                if !last_message_is_ui_resource(&msgs) {
                    let _ = manager.resume_session(&session_id).await;
                    let _ = manager.resume_workflow(session_id.clone()).await;
                    sleep(Duration::from_millis(500)).await;
                }
            }

            let wait_result = {
                let gate = crate::state::get_concurrency_gate();
                let mut active_permit = Some(
                    manager
                        .take_active_session_permit(&caller_session_id)
                        .await
                        .ok_or_else(|| {
                            format!(
                                "Caller session {} is not holding an active concurrency permit",
                                caller_session_id
                            )
                        })?,
                );
                let suspended = match gate.suspend_agent(&mut active_permit).await {
                    Ok(suspended) => suspended,
                    Err(error) => {
                        if let Some(permit) = active_permit.take() {
                            manager
                                .restore_active_session_permit(&caller_session_id, permit)
                                .await?;
                        }
                        return Err(error);
                    }
                };
                let res = wait_until_session_terminal(
                    manager,
                    &session_id,
                    timeout_seconds,
                    Some(&caller_session_id),
                )
                .await;
                let resumed = suspended.resume().await?;
                manager
                    .restore_active_session_permit(&caller_session_id, resumed)
                    .await?;
                res
            };

            let (session_data, _poll_count) = match handle_wait_timeout_result(
                wait_result,
                Some(manager),
                &session_id,
                timeout_seconds,
                "awaitAgent",
                false,
            )
            .await
            {
                Ok(res) => res,
                Err(mcp_res) => return mcp_res,
            };

            let turn_count = count_session_turns(&session_id).await;
            let messages_data = fetch_messages_value(&session_id, result_message_limit).await?;
            let messages = messages_data
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let (message_id, assistant_text) =
                latest_assistant_message_text(&messages, assistant_message_max_chars)
                    .unwrap_or(("none".to_string(), "No assistant result found.".to_string()));

            Ok(success_result(
                format!(
                    "Session {} reached terminal status '{}'.\n\nLatest result [{}]:\n{}",
                    session_id,
                    extract_session_status(&session_data),
                    message_id,
                    assistant_text
                ),
                SwarmOperationResponse {
                    operation: "await".to_string(),
                    session: map_session_response(&session_data, turn_count, Some(assistant_text)),
                    messages_count: Some(messages.len()),
                },
            ))
        }
        "messageAgent" => {
            let manager = manager.ok_or_else(|| {
                "AgentSessionManager not available for legacy session API tools".to_string()
            })?;
            let session_id = read_required_string(&args, "sessionId")?;
            let content = read_required_string(&args, "content")?;

            let response = match crate::services::AgentService::send_message_to_session(
                manager,
                &session_id,
                content,
                Some(MessageSource::SwarmLegacy),
            )
            .await
            {
                Ok(response) => response,
                Err(e) if e.contains("Session not found:") => {
                    return Ok(session_not_found_error("Message Agent", &session_id))
                }
                Err(e) => return Err(e),
            };

            Ok(success_result(
                format!(
                    "Message accepted by session {} (ID: {})",
                    session_id, response.message_id
                ),
                json!({ "messageId": response.message_id, "status": response.status }),
            ))
        }
        "terminateAgent" => {
            let manager = manager.ok_or_else(|| {
                "AgentSessionManager not available for legacy session API tools".to_string()
            })?;
            let session_id = read_required_string(&args, "sessionId")?;

            if caller_session_id.as_deref() == Some(session_id.as_str()) {
                return Ok(swarm_error(
                    ErrorCategory::PermissionDenied,
                    "Terminate Agent",
                    "Self-termination is not allowed.".to_string(),
                    vec!["Use planning tool to mark task complete instead".to_string()],
                ));
            }

            match manager.terminate_session(session_id.clone()).await {
                Ok(_) => {
                    crate::services::agent_service::remove_lineage(&session_id).await;
                    Ok(success_result(
                        format!("Terminated session: {}", session_id),
                        json!({ "sessionId": session_id, "terminated": true }),
                    ))
                }
                Err(e) if e.contains("not found") => {
                    Ok(session_not_found_error("Terminate Agent", &session_id))
                }
                Err(e) => Err(e),
            }
        }
        "getAgentLog" => {
            let manager = manager.ok_or_else(|| {
                "AgentSessionManager not available for legacy session API tools".to_string()
            })?;
            let target_session_id = read_required_string(&args, "sessionId")?;
            let requested_limit = args.get("limit").and_then(|v| v.as_u64());
            let options = read_message_summary_options(&args);

            let rapid_poll_hint = if options.skip_if_unchanged {
                min_interval_notice(
                    caller_session_id.as_deref(),
                    &target_session_id,
                    requested_limit,
                    options,
                )
                .await
            } else {
                None
            };

            if fetch_session_value(manager, &target_session_id)
                .await?
                .is_none()
            {
                return Ok(session_not_found_error("Get Agent Log", &target_session_id));
            }

            let data =
                fetch_messages_value(&target_session_id, requested_limit.unwrap_or(50)).await?;

            let messages = data
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if options.skip_if_unchanged {
                if let Some(unchanged_notice) = unchanged_messages_notice(
                    &messages,
                    caller_session_id.as_deref(),
                    &target_session_id,
                    requested_limit,
                )
                .await
                {
                    return Ok(success_result(unchanged_notice, data));
                }
            }

            let summary_text = build_messages_summary(&messages, &target_session_id, options);
            let final_text = match rapid_poll_hint {
                Some(hint) => format!("{}\n\n---\n\n{}", hint, summary_text),
                None => summary_text,
            };

            Ok(success_result(final_text, data))
        }
        "getChildAgents" => {
            let parent_session_id = caller_session_id
                .clone()
                .ok_or_else(|| "getChildAgents requires a caller session context".to_string())?;
            let session_repo = crate::state::get_session_repository();
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            let child_ids = session_repo
                .get_child_session_ids(&parent_session_id)
                .await
                .map_err(|e| format!("Failed to fetch child sessions: {}", e))?;

            let total_count = child_ids.len();

            let data = json!({
                "parentSessionId": parent_session_id,
                "count": total_count,
                "children": child_ids,
            });

            let child_ids = data
                .get("children")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .skip(offset)
                .take(limit)
                .collect::<Vec<_>>();

            let mut session_responses: Vec<AgentSessionResponse> = Vec::new();

            for child_id in &child_ids {
                // Fetch each child session data to map it properly
                if let Ok(Some(session)) = session_repo.get_session(child_id).await {
                    let child_data = match session_metadata_to_value(&session) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    let turn_count = count_session_turns(child_id).await;
                    let preview = latest_assistant_preview_for_session(
                        child_id,
                        SWARM_MESSAGE_PREVIEW_MAX_CHARS,
                    )
                    .await;
                    session_responses.push(map_session_response(&child_data, turn_count, preview));
                }
            }

            let mut message = format!(
                "Fetched {} direct sub-agents for commander session {}",
                session_responses.len(),
                parent_session_id
            );

            if session_responses.is_empty() {
                message.push_str(
                    "\n\nNo direct sub-agents online. Next step: use spawnAgent to deploy a worker.",
                );
            } else {
                message.push_str("\n\nDirect unit roster:\n");
                message.push_str("| Name | ID | Status | Turns | Latest Result |\n");
                message.push_str("|---|---|---|---|---|\n");
                for resp in &session_responses {
                    let latest = resp
                        .latest_result
                        .as_deref()
                        .unwrap_or("-")
                        .replace('\n', " ")
                        .replace('|', "\\|");
                    message.push_str(&format!(
                        "| {} | `{}` | {} | {} | {} |\n",
                        resp.name.replace('\n', " ").replace('|', "\\|"),
                        resp.id,
                        resp.status,
                        resp.turn_count,
                        latest
                    ));
                }

                let has_more = offset + session_responses.len() < total_count;
                if has_more {
                    message.push_str(&format!(
                        "\n*(Showing {} to {} of {} items. Call this tool again with offset: {} to see more)*",
                        offset + 1,
                        offset + session_responses.len(),
                        total_count,
                        offset + limit
                    ));
                } else if offset > 0 {
                    message.push_str(&format!(
                        "\n*(Showing {} to {} of {} items)*",
                        offset + 1,
                        offset + session_responses.len(),
                        total_count
                    ));
                }
            }

            Ok(success_result(message, session_responses))
        }
        "listAgentTypes" => {
            let assistant_repo = crate::state::get_assistant_repository();
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            let assistants = assistant_repo
                .list_assistants()
                .await
                .map_err(|e| format!("Failed to list assistants: {}", e))?;
            let data = serde_json::to_value(&assistants)
                .map_err(|e| format!("Failed to serialize assistants: {}", e))?;
            let assistants_array = data
                .as_array()
                .cloned()
                .or_else(|| data.get("assistants").and_then(|v| v.as_array()).cloned())
                .or_else(|| data.get("items").and_then(|v| v.as_array()).cloned())
                .unwrap_or_default();

            let total_count = assistants_array.len();
            let paginated_assistants: Vec<_> = assistants_array
                .into_iter()
                .skip(offset)
                .take(limit)
                .collect();

            let mut message = if paginated_assistants.is_empty() {
                "No assistant types available.".to_string()
            } else {
                let mut lines = vec![
                    format!(
                        "Available assistant types (showing {} to {} of {}):",
                        offset + 1,
                        offset + paginated_assistants.len(),
                        total_count
                    ),
                    "\n| Name | ID | Model | Description |".to_string(),
                    "|---|---|---|---|".to_string(),
                ];

                for assistant in &paginated_assistants {
                    let id = assistant
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let name = assistant
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed")
                        .replace('\n', " ")
                        .replace('|', "\\|");

                    let config = assistant.get("config").cloned().unwrap_or(json!({}));
                    let parsed_config = if let Some(s) = config.as_str() {
                        serde_json::from_str::<Value>(s).unwrap_or(json!({}))
                    } else {
                        config
                    };
                    let description = extract_assistant_description(&parsed_config)
                        .replace('\n', " ")
                        .replace('|', "\\|");
                    let model = parsed_config
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .replace('\n', " ")
                        .replace('|', "\\|");

                    lines.push(format!(
                        "| {} | `{}` | {} | {} |",
                        name, id, model, description
                    ));
                }

                lines.join("\n")
            };

            let has_more = offset + paginated_assistants.len() < total_count;
            if has_more {
                message.push_str(&format!(
                    "\n\n*(Showing {} to {} of {} items. Call this tool again with offset: {} to see more)*",
                    offset + 1,
                    offset + paginated_assistants.len(),
                    total_count,
                    offset + limit
                ));
            } else if offset > 0 {
                message.push_str(&format!(
                    "\n\n*(Showing {} to {} of {} items)*",
                    offset + 1,
                    offset + paginated_assistants.len(),
                    total_count
                ));
            }

            Ok(success_result(message, data))
        }
        "getAgentConfig" => {
            let assistant_id = read_required_string(&args, "assistantId")?;
            let assistant_repo = crate::state::get_assistant_repository();
            let assistant = assistant_repo
                .get_assistant(&assistant_id)
                .await
                .map_err(|e| format!("Failed to fetch assistant: {}", e))?
                .ok_or_else(|| format!("Assistant '{}' not found\n\nSuggestion: Use listAgentTypes to see available assistant IDs.", assistant_id))?;
            let data = serde_json::to_value(&assistant)
                .map_err(|e| format!("Failed to serialize assistant config: {}", e))?;

            let name = data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let config = data.get("config").cloned().unwrap_or(json!({}));
            let parsed_config = if let Some(s) = config.as_str() {
                serde_json::from_str::<Value>(s).unwrap_or(json!({}))
            } else {
                config
            };

            let description = extract_assistant_description(&parsed_config);
            let system_prompt = parsed_config
                .get("systemPrompt")
                .and_then(|v| v.as_str())
                .unwrap_or("(none)");
            let model = parsed_config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            Ok(success_result(
                format!(
                    "Assistant: {} [ID: {}]\nModel: {}\nDescription: {}\nSystem Prompt:\n{}",
                    name, assistant_id, model, description, system_prompt
                ),
                data,
            ))
        }
        _ => Err(format!("Unknown tool: {}", tool_name)),
    }
}
