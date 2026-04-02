use serde_json::{json, Value};
use std::sync::Arc;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_agent_config_error, missing_agent_session_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::builtin::session_api::formatting::{
    extract_session_status, is_terminal_status, latest_assistant_message_text,
    latest_tool_message_text,
};
use crate::mcp::builtin::session_api::utils::{
    build_agent_session_tool_data, build_agent_tool_data, check_session_next_actions,
    count_session_turns, handle_wait_timeout_result, read_required_string,
    wait_until_session_terminal,
};
use crate::mcp::types::{MCPContent, MCPResult};
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::repositories::message_repository::MessageRepository;

use super::formatting::{
    build_server_name_lookup, extract_string_list, format_capability_list,
    format_external_server_refs, resolve_external_server_labels,
};
use super::AgentServer;

fn extract_result_text(result: &MCPResult) -> Option<String> {
    result
        .content
        .as_ref()?
        .iter()
        .find_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
}

fn normalize_agent_config_result(
    mut result: MCPResult,
    tool_name: &str,
    next_actions: Vec<Value>,
) -> MCPResult {
    if result.is_error == Some(true) {
        return result;
    }

    let message =
        extract_result_text(&result).unwrap_or_else(|| format!("{} completed.", tool_name));
    let existing = result.structured_content.take();
    let resource_id = existing
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|object| object.get("id"))
        .and_then(|value| value.as_str());

    let mut data = build_agent_tool_data(
        tool_name,
        "agentConfig",
        resource_id,
        &message,
        "success",
        next_actions,
    );

    match existing {
        Some(Value::Object(object)) => {
            for (key, value) in object {
                data.insert(key, value);
            }
        }
        Some(value) => {
            data.insert("data".to_string(), value);
        }
        None => {}
    }

    result.structured_content = Some(Value::Object(data));
    result
}

fn default_session_recovery_message(status: &str) -> String {
    match status.to_ascii_lowercase().as_str() {
        "paused" => "Resume after interruption. Continue the delegated task from the last completed step, reassess any interrupted tool work, and then proceed to the final answer.".to_string(),
        "error" | "failed" => "Retry after failure. Inspect the previous error, preserve any completed work, fix the immediate cause if needed, and continue the delegated task from the safest next step.".to_string(),
        "terminated" => "Restart the delegated task from the latest conversation state and continue from the most sensible next step.".to_string(),
        _ => "Continue the delegated task from the latest stable point and report the final answer.".to_string(),
    }
}

fn recovery_action_for_session(session_id: &str, status: &str, reason: &str) -> Value {
    json!({
        "toolName": "messageToSession",
        "reason": reason,
        "args": {
            "sessionId": session_id,
            "message": default_session_recovery_message(status),
        }
    })
}

fn latest_session_output(messages_value: &[Value]) -> String {
    let (_, mut assistant_text) = latest_assistant_message_text(messages_value, None)
        .unwrap_or(("none".to_string(), "No final answer yet.".to_string()));

    if assistant_text == "[assistant message has no text content]" {
        if let Some(tool_text) = latest_tool_message_text(messages_value) {
            assistant_text = format!("[Tool Response Fallback]\n{}", tool_text);
        }
    }

    assistant_text
}

pub fn build_paused_check_session_result_from_messages(
    session_id: &str,
    turn_count: usize,
    messages_value: &[Value],
) -> MCPResult {
    let latest_output = latest_session_output(messages_value);
    let recovery_reason =
        "Wake the paused child session so it can continue from the last stable step.";
    let message = format!(
        "Session {} is paused and will not make progress on its own.\n\nLast known output:\n{}\n\nRecovery: send a follow-up message with messageToSession(...) to restart the child workflow.",
        session_id, latest_output
    );
    let next_actions = vec![
        recovery_action_for_session(session_id, "paused", recovery_reason),
        json!({
            "toolName": "checkSession",
            "reason": "Check again after sending a recovery message.",
            "args": {
                "sessionId": session_id,
                "wait": true
            }
        }),
    ];
    let hint = SuccessHint::new(message.clone(), vec![]);
    let mut response_data = build_agent_session_tool_data(
        "checkSession",
        session_id,
        &message,
        "paused",
        "paused",
        turn_count,
        next_actions,
    );
    response_data.insert("recoverable".to_string(), Value::Bool(true));
    response_data.insert(
        "recoveryStrategy".to_string(),
        Value::String("messageToSession".to_string()),
    );
    response_data.insert(
        "recoveryMessage".to_string(),
        Value::String(default_session_recovery_message("paused")),
    );
    response_data.insert("abnormalTermination".to_string(), Value::Bool(false));
    response_data.insert("result".to_string(), Value::String(latest_output));

    hint.to_mcp_result_with_data(Some(Value::Object(response_data)))
}

pub fn build_terminal_check_session_result_from_messages(
    session_id: &str,
    status: &str,
    turn_count: usize,
    messages_value: &[Value],
) -> MCPResult {
    let assistant_text = latest_session_output(messages_value);
    let normalized_status = status.to_ascii_lowercase();
    let is_abnormal = matches!(normalized_status.as_str(), "error" | "failed");
    let is_recoverable = matches!(
        normalized_status.as_str(),
        "error" | "failed" | "terminated"
    );
    let next_actions = if is_recoverable {
        vec![
            recovery_action_for_session(
                session_id,
                status,
                "Retry the child session explicitly after abnormal termination.",
            ),
            json!({
                "toolName": "checkSession",
                "reason": "Check again after sending a recovery message.",
                "args": {
                    "sessionId": session_id,
                    "wait": true
                }
            }),
        ]
    } else {
        vec![]
    };
    let message = if is_abnormal {
        format!(
            "Session {} ended abnormally ({}).\n\nLast known output:\n{}\n\nRecovery: this child session will not continue on its own. Use messageToSession(...) to retry from the last stable step.",
            session_id, status, assistant_text
        )
    } else if normalized_status == "terminated" {
        format!(
            "Session {} was terminated.\n\nLast known output:\n{}\n\nIf you still need the work, restart it explicitly with messageToSession(...).",
            session_id, assistant_text
        )
    } else {
        format!(
            "Session {} is terminal ({}).\n\nResult:\n{}",
            session_id, status, assistant_text
        )
    };
    let hint = SuccessHint::new(message.clone(), vec![]);
    let mut response_data = build_agent_session_tool_data(
        "checkSession",
        session_id,
        &message,
        status,
        if is_abnormal {
            "error"
        } else if normalized_status == "terminated" {
            "terminated"
        } else {
            "success"
        },
        turn_count,
        next_actions,
    );
    response_data.insert("result".to_string(), Value::String(assistant_text));
    response_data.insert("abnormalTermination".to_string(), Value::Bool(is_abnormal));
    response_data.insert("recoverable".to_string(), Value::Bool(is_recoverable));
    if is_recoverable {
        response_data.insert(
            "recoveryStrategy".to_string(),
            Value::String("messageToSession".to_string()),
        );
        response_data.insert(
            "recoveryMessage".to_string(),
            Value::String(default_session_recovery_message(status)),
        );
    }

    hint.to_mcp_result_with_data(Some(Value::Object(response_data)))
}

async fn build_terminal_check_session_result(
    session_id: &str,
    status: &str,
    turn_count: usize,
) -> Result<MCPResult, String> {
    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, 5)
        .await
        .map_err(|e| format!("Failed to fetch session messages: {}", e))?;

    let messages_value: Vec<Value> = messages
        .into_iter()
        .map(|m| serde_json::to_value(m).unwrap_or_default())
        .collect();

    Ok(build_terminal_check_session_result_from_messages(
        session_id,
        status,
        turn_count,
        &messages_value,
    ))
}

async fn build_paused_check_session_result(
    session_id: &str,
    turn_count: usize,
) -> Result<MCPResult, String> {
    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, 5)
        .await
        .map_err(|e| format!("Failed to fetch session messages: {}", e))?;

    let messages_value: Vec<Value> = messages
        .into_iter()
        .map(|m| serde_json::to_value(m).unwrap_or_default())
        .collect();

    Ok(build_paused_check_session_result_from_messages(
        session_id,
        turn_count,
        &messages_value,
    ))
}

/// Unified create_agent handler (from createAssistant)
pub async fn create_agent(server: &AgentServer, args: Value) -> Result<MCPResult, String> {
    let mut mapped_args = args.clone();

    // Map Agent Domain friendly names to underlying config fields
    if let Some(builtins) = args.get("builtinCapabilities") {
        mapped_args["allowedBuiltInServiceAliases"] = builtins.clone();
    }
    if let Some(externals) = args.get("externalMcpServers") {
        mapped_args["mcpServerIds"] = externals.clone();
    }

    let assistant_server =
        crate::mcp::builtin::assistant::AssistantServer::new(Arc::new(server.get_db().clone()))
            .await?;
    let result = crate::mcp::builtin::assistant::operations::create_assistant(
        &assistant_server,
        mapped_args,
    )
    .await?;
    Ok(normalize_agent_config_result(
        result,
        "create",
        vec![json!({
            "toolName": "list",
            "reason": "Review the available agent configurations after creating this one.",
            "args": {
                "type": "configs"
            }
        })],
    ))
}

/// Unified update_agent handler (from updateAssistant)
pub async fn update_agent(
    server: &AgentServer,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let mut mapped_args = args.clone();

    // Map Agent Domain friendly names to underlying config fields
    if let Some(builtins) = args.get("builtinCapabilities") {
        mapped_args["allowedBuiltInServiceAliases"] = builtins.clone();
    }
    if let Some(externals) = args.get("externalMcpServers") {
        mapped_args["mcpServerIds"] = externals.clone();
    }

    let assistant_server =
        crate::mcp::builtin::assistant::AssistantServer::new(Arc::new(server.get_db().clone()))
            .await?;
    let result = crate::mcp::builtin::assistant::operations::update_assistant(
        &assistant_server,
        mapped_args,
        caller_session_id,
    )
    .await?;
    Ok(normalize_agent_config_result(
        result,
        "update",
        vec![json!({
            "toolName": "list",
            "reason": "Review the updated agent configurations after this change.",
            "args": {
                "type": "configs"
            }
        })],
    ))
}

/// Unified list handler: lists configs or sub-sessions
pub async fn list_agents_or_sessions(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let list_type = args
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("configs");

    match list_type {
        "configs" => {
            use crate::repositories::AssistantRepository;
            let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());
            let mut agents = repo.list_assistants().await.map_err(|e| e.to_string())?;

            // Filter by query if provided
            if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
                let q = query.to_lowercase();
                agents.retain(|a| {
                    a.name.to_lowercase().contains(&q) || a.config.to_lowercase().contains(&q)
                });
            }

            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            let total = agents.len();
            let paged_agents: Vec<_> = agents.into_iter().skip(offset).take(limit).collect();
            let mcp_repo = crate::state::get_mcp_server_repository();
            let external_servers = mcp_repo.list().await.map_err(|e| e.to_string())?;
            let server_name_lookup = build_server_name_lookup(&external_servers);

            let mut results = Vec::new();
            let mut text_summary = format!("Found {} agent configurations.\n\n", total);

            for agent in paged_agents {
                let config: Value = serde_json::from_str(&agent.config).unwrap_or_default();
                let desc = config
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No description");
                let builtins = extract_string_list(config.get("allowedBuiltInServiceAliases"));
                let external_ids = extract_string_list(config.get("mcpServerIds"));
                let external_labels =
                    resolve_external_server_labels(&external_ids, &server_name_lookup);

                text_summary.push_str(&format!(
                    "- **{}** (ID: `{}`)\n  Description: {}\n  Builtin Capabilities: {}\n  External MCP Servers: {}\n\n",
                    agent.name,
                    agent.id,
                    desc,
                    format_capability_list(&builtins),
                    format_external_server_refs(&external_ids, &server_name_lookup)
                ));

                results.push(json!({
                    "id": agent.id,
                    "name": agent.name,
                    "description": desc,
                    "builtinCapabilities": builtins,
                    "externalMcpServers": external_ids,
                    "externalMcpServerLabels": external_labels
                }));
            }

            let hint = SuccessHint::new(
                text_summary,
                vec!["Use startSession(agentId=\"...\") to delegate work".to_string()],
            );
            let response_message = hint.message.clone();
            let mut response_data = build_agent_tool_data(
                "list",
                "agentConfigCollection",
                None,
                &response_message,
                "success",
                vec![json!({
                    "toolName": "startSession",
                    "reason": "Start a delegated session with one of the listed agent configurations.",
                })],
            );
            response_data.insert("type".to_string(), Value::String("configs".to_string()));
            response_data.insert("agents".to_string(), Value::Array(results));
            response_data.insert("total".to_string(), json!(total));
            Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
        }
        "sessions" => {
            // Logic from getChildAgents
            let session_repo = crate::state::get_session_repository();
            use crate::repositories::session_repository::SessionRepository;

            let child_ids = match session_repo.get_child_session_ids(caller_session_id).await {
                Ok(ids) => ids,
                Err(_) => {
                    // Fallback to lineage store if DB fails or doesn't have the relationship yet
                    let store = crate::services::agent_service::lineage_store().read().await;
                    store
                        .iter()
                        .filter_map(|(id, meta)| {
                            if meta.parent_session_id.as_deref() == Some(caller_session_id) {
                                Some(id.clone())
                            } else {
                                None
                            }
                        })
                        .collect()
                }
            };

            let mut results = Vec::new();
            for child_id in &child_ids {
                if let Ok(Some(child_data)) = session_repo.get_session(child_id).await {
                    let status = format!("{:?}", child_data.status).to_lowercase();
                    results.push(json!({
                        "id": child_id,
                        "name": child_data.name.unwrap_or_else(|| "Unnamed".to_string()),
                        "status": status
                    }));
                }
            }

            let mut message = format!("Found {} sub-agent sessions.", results.len());
            if !results.is_empty() {
                message.push_str("\n\nActive roster:\n");
                for r in &results {
                    message.push_str(&format!(
                        "- {} (ID: {}) status={}\n",
                        r["name"], r["id"], r["status"]
                    ));
                }
            }

            let hint = SuccessHint::new(
                message,
                vec!["Use checkSession(sessionId) to get results".to_string()],
            );
            let response_message = hint.message.clone();
            let mut response_data = build_agent_tool_data(
                "list",
                "sessionCollection",
                None,
                &response_message,
                "success",
                vec![json!({
                    "toolName": "checkSession",
                    "reason": "Inspect one of the listed delegated sessions in more detail.",
                })],
            );
            response_data.insert("type".to_string(), Value::String("sessions".to_string()));
            response_data.insert("sessions".to_string(), Value::Array(results));
            response_data.insert("total".to_string(), json!(child_ids.len()));
            Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
        }
        _ => Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Invalid list type '{}'. Use 'configs' or 'sessions'.",
                list_type
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Use list(type=\"configs\") to see agent configurations".to_string(),
            "Use list(type=\"sessions\") to inspect delegated sub-agent sessions".to_string(),
        ])
        .to_mcp_result()),
    }
}

/// startSession handler (from spawnAgent)
pub async fn start_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;

    let body: crate::agent::types::CreateSessionRequest = serde_json::from_value(json!({
        "parentSessionId": caller_session_id,
        "assistantId": read_required_string(&args, "agentId")?,
        "request": read_required_string(&args, "task")?,
        "workspacePath": args.get("workspaceOverride").and_then(|v| v.as_str()),
        "maxDepth": args.get("maxDepth").and_then(|v| v.as_u64()),
        "maxFanout": args.get("maxFanout").and_then(|v| v.as_u64()),
    }))
    .map_err(|e| format!("Invalid arguments for start_session: {}", e))?;

    let wait_for_result = args
        .get("waitForResult")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let response = match crate::services::AgentService::spawn_agent(manager, body).await {
        Ok(res) => res,
        Err(err) if err.contains("Assistant not found:") => {
            let agent_id = args
                .get("agentId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Ok(missing_agent_config_error(agent_id));
        }
        Err(err) => return Err(err),
    };

    let session_id = response.id;

    if wait_for_result {
        return check_session(
            server,
            json!({ "sessionId": session_id, "wait": true }),
            caller_session_id,
        )
        .await;
    }

    let hint = SuccessHint::new(
        format!("Session started successfully (ID: {}).", session_id),
        vec![format!(
            "Use checkSession(\"{}\", wait=true) to wait for the answer.",
            session_id
        )],
    );
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "startSession",
        "session",
        Some(&session_id),
        &message,
        "pending",
        check_session_next_actions(&session_id),
    );
    response_data.insert("sessionId".to_string(), Value::String(session_id.clone()));
    response_data.insert("status".to_string(), Value::String("started".to_string()));

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}

/// messageToSession handler (from messageAgent)
pub async fn message_to_session(
    server: &AgentServer,
    args: Value,
    _caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;
    let message_text = read_required_string(&args, "message")?;
    let response = match crate::services::AgentService::send_message_to_session(
        manager,
        &session_id,
        message_text,
        Some("agent_tool".to_string()),
    )
    .await
    {
        Ok(response) => response,
        Err(err) if err.contains("Session not found:") => {
            return Ok(missing_agent_session_error(&session_id));
        }
        Err(err) => return Err(err),
    };

    let hint = SuccessHint::new(
        format!("Message {} for session {}.", response.status, session_id),
        vec![format!(
            "Use checkSession(\"{}\", wait=true) to see the response.",
            session_id
        )],
    );
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "messageToSession",
        "session",
        Some(&session_id),
        &message,
        "pending",
        check_session_next_actions(&session_id),
    );
    response_data.insert("sessionId".to_string(), Value::String(session_id));
    response_data.insert("messageId".to_string(), Value::String(response.message_id));
    response_data.insert("status".to_string(), Value::String(response.status));

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}

/// checkSession handler (from awaitAgent / getAgentStatus)
pub async fn check_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;
    let wait = args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
    let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(3600);

    let current_session_meta = manager
        .get_session(&session_id)
        .await?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    let current_status = format!("{:?}", current_session_meta.status).to_lowercase();
    let current_turn_count = count_session_turns(&session_id).await;

    if current_status == "paused" {
        return build_paused_check_session_result(&session_id, current_turn_count).await;
    }

    if wait {
        let wait_result = {
            let gate = crate::state::get_concurrency_gate();
            let active_permit = manager
                .take_active_session_permit(caller_session_id)
                .await
                .ok_or_else(|| {
                    format!(
                        "Caller session {} is not holding an active concurrency permit",
                        caller_session_id
                    )
                })?;
            let suspended = gate.suspend_agent(active_permit).await?;
            let res = wait_until_session_terminal(
                manager,
                &session_id,
                timeout_secs,
                Some(caller_session_id),
            )
            .await;
            let resumed = suspended.resume().await?;
            manager
                .restore_active_session_permit(caller_session_id, resumed)
                .await?;
            res
        };

        let (session_data, _) = match handle_wait_timeout_result(
            wait_result,
            Some(manager),
            &session_id,
            timeout_secs,
            "checkSession",
            false,
        )
        .await
        {
            Ok(res) => res,
            Err(mcp_res) => return mcp_res,
        };

        let status = extract_session_status(&session_data);
        let turn_count = count_session_turns(&session_id).await;
        if status == "paused" {
            return build_paused_check_session_result(&session_id, turn_count).await;
        }
        return build_terminal_check_session_result(&session_id, &status, turn_count).await;
    }

    // Just check status via manager
    let status = current_status;
    let turn_count = current_turn_count;

    let is_terminal = is_terminal_status(&status);
    if is_terminal {
        return build_terminal_check_session_result(&session_id, &status, turn_count).await;
    }

    let next_steps = vec![format!(
        "Use checkSession(\"{}\", wait=true) to wait for completion.",
        session_id
    )];
    let message = format!(
        "Session {} is currently {} (Turns elapsed: {}).",
        session_id, status, turn_count
    );
    let hint = SuccessHint::new(message.clone(), next_steps);
    let response_status = "pending";
    let next_actions = check_session_next_actions(&session_id);

    Ok(
        hint.to_mcp_result_with_data(Some(Value::Object(build_agent_session_tool_data(
            "checkSession",
            &session_id,
            &message,
            &status,
            response_status,
            turn_count,
            next_actions,
        )))),
    )
}

/// stopSession handler (from terminateAgent)
pub async fn stop_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;

    if caller_session_id == session_id {
        return Ok(guided_error(
            ErrorCategory::InvalidState,
            "Self-termination is not allowed via stopSession.",
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Use stopSession only for child or delegated sessions".to_string(),
            "If the current workflow should stop, use the normal session cancellation controls instead"
                .to_string(),
        ])
        .to_mcp_result());
    }

    if let Err(e) = manager.terminate_session(session_id.clone()).await {
        if e.contains("not found") {
            return Ok(missing_agent_session_error(&session_id));
        }
        return Err(e);
    }

    // Also remove from lineage store if present
    crate::services::agent_service::lineage_store()
        .write()
        .await
        .remove(&session_id);

    let hint = SuccessHint::new(format!("Session {} stopped.", session_id), vec![]);
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "stopSession",
        "session",
        Some(&session_id),
        &message,
        "success",
        vec![],
    );
    response_data.insert("sessionId".to_string(), Value::String(session_id));
    response_data.insert("stopped".to_string(), Value::Bool(true));
    response_data.insert(
        "status".to_string(),
        Value::String("terminated".to_string()),
    );

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
