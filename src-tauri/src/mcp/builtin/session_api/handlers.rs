use reqwest::Method;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

use crate::mcp::builtin::error_guidance::ErrorCategory;
use crate::mcp::types::MCPResult;

use super::cache::{min_interval_notice, unchanged_messages_notice};
use super::client::call_json;
use super::formatting::*;
use super::types::*;
use super::utils::*;

/// Helper to map raw session JSON to the standardized `AgentSessionResponse`.
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
        assistant_id: session
            .get("assistantId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        turn_count,
        latest_result,
    }
}

pub async fn handle_tool_call(
    tool_name: &str,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    match tool_name {
        "healthCheck" => {
            let data = call_json(Method::GET, "/api/health", None, None).await?;
            Ok(success_result(
                "Session API health check succeeded.".to_string(),
                data,
            ))
        }
        "spawnAgent" => {
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

            let await_completion = args
                .get("awaitCompletion")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let timeout_seconds = args
                .get("timeoutSeconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(180);

            let data = call_json(Method::POST, "/api/sessions", Some(body), None)
                .await
                .map_err(|e| {
                    if e.contains("404") && e.contains("Assistant") {
                        return format!(
                            "Assistant '{}' not found. Use listAgentTypes to see available types.",
                            assistant_id
                        );
                    }
                    e
                })?;

            let child_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let child_id_owned = child_id.to_string();

            if !await_completion {
                return Ok(success_result(
                    format!(
                        "Child agent '{}' spawned successfully (ID: {}).\n\nUse awaitAgent(\"{}\") to wait for completion and fetch results.",
                        data.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed"),
                        child_id_owned, child_id_owned
                    ),
                    SwarmOperationResponse {
                        operation: "spawn".to_string(),
                        session: map_session_response(&data, 0, None),
                        messages_count: None,
                    },
                ));
            }

            // awaitCompletion=true: SP2 two-phase transition + SP1 push-notify wait.
            let wait_result = {
                let gate = crate::state::get_concurrency_gate();
                gate.suspend_agent().await?;
                let res = wait_until_session_terminal(
                    &child_id_owned,
                    timeout_seconds,
                    caller_session_id.as_deref(),
                )
                .await;
                gate.resume_agent().await?;
                res
            };

            let (session_data, poll_count) = match handle_wait_timeout_result(
                wait_result,
                &child_id_owned,
                timeout_seconds,
                true,
            ) {
                Ok(res) => res,
                Err(mcp_res) => return mcp_res,
            };

            let final_status = extract_session_status(&session_data);
            let turn_count = count_session_turns(&child_id_owned).await;

            let messages_data = call_json(
                Method::GET,
                &format!("/api/sessions/{}/messages", child_id_owned),
                None,
                Some(vec![("limit".to_string(), "20".to_string())]),
            )
            .await?;

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
            let session_id = read_required_string(&args, "sessionId")?;
            let data = match call_json(
                Method::GET,
                &format!("/api/sessions/{}", session_id),
                None,
                None,
            )
            .await
            {
                Ok(data) => data,
                Err(e) if e.contains("404") => {
                    return Ok(session_not_found_error("Get Agent Status", &session_id))
                }
                Err(e) => return Err(e),
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
            let session_id = read_required_string(&args, "sessionId")?;

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
            let initial_session = match call_json(
                Method::GET,
                &format!("/api/sessions/{}", session_id),
                None,
                None,
            )
            .await
            {
                Ok(data) => data,
                Err(e) if e.contains("404") => {
                    return Ok(session_not_found_error("Await Agent", &session_id))
                }
                Err(e) => return Err(e),
            };

            if extract_session_status(&initial_session) == "paused" {
                // ... (auto-resume logic remains same but structured)
                let msgs_data = call_json(
                    Method::GET,
                    &format!("/api/sessions/{}/messages", session_id),
                    None,
                    Some(vec![("limit".to_string(), "5".to_string())]),
                )
                .await
                .unwrap_or_default();
                let msgs = msgs_data
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                if !last_message_is_ui_resource(&msgs) {
                    let _ = call_json(
                        Method::POST,
                        &format!("/api/sessions/{}/resume", session_id),
                        None,
                        None,
                    )
                    .await;
                    sleep(Duration::from_millis(500)).await;
                }
            }

            let wait_result = {
                let gate = crate::state::get_concurrency_gate();
                gate.suspend_agent().await?;
                let res = wait_until_session_terminal(
                    &session_id,
                    timeout_seconds,
                    caller_session_id.as_deref(),
                )
                .await;
                gate.resume_agent().await?;
                res
            };

            let (session_data, _poll_count) = match handle_wait_timeout_result(
                wait_result,
                &session_id,
                timeout_seconds,
                false,
            ) {
                Ok(res) => res,
                Err(mcp_res) => return mcp_res,
            };

            let turn_count = count_session_turns(&session_id).await;
            let messages_data = call_json(
                Method::GET,
                &format!("/api/sessions/{}/messages", session_id),
                None,
                Some(vec![(
                    "limit".to_string(),
                    result_message_limit.to_string(),
                )]),
            )
            .await?;
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
            let session_id = read_required_string(&args, "sessionId")?;
            let content = read_required_string(&args, "content")?;

            let data = match call_json(
                Method::POST,
                &format!("/api/sessions/{}/messages", session_id),
                Some(json!({ "content": content })),
                None,
            )
            .await
            {
                Ok(data) => data,
                Err(e) if e.contains("404") => {
                    return Ok(session_not_found_error("Message Agent", &session_id))
                }
                Err(e) => return Err(e),
            };

            Ok(success_result(
                format!(
                    "Message accepted by session {} (ID: {})",
                    session_id,
                    data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown")
                ),
                json!({ "messageId": data.get("id"), "status": data.get("status") }),
            ))
        }
        "terminateAgent" => {
            let session_id = read_required_string(&args, "sessionId")?;

            if caller_session_id.as_deref() == Some(session_id.as_str()) {
                return Ok(swarm_error(
                    ErrorCategory::PermissionDenied,
                    "Terminate Agent",
                    "Self-termination is not allowed.".to_string(),
                    vec!["Use planning tool to mark task complete instead".to_string()],
                ));
            }

            match call_json(
                Method::POST,
                &format!("/api/sessions/{}/terminate", session_id),
                None,
                None,
            )
            .await
            {
                Ok(_) => Ok(success_result(
                    format!("Terminated session: {}", session_id),
                    json!({ "sessionId": session_id, "terminated": true }),
                )),
                Err(e) if e.contains("404") => {
                    Ok(session_not_found_error("Terminate Agent", &session_id))
                }
                Err(e) => Err(e),
            }
        }
        "getAgentLog" => {
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

            let query = requested_limit.map(|v| vec![("limit".to_string(), v.to_string())]);
            let data = match call_json(
                Method::GET,
                &format!("/api/sessions/{}/messages", target_session_id),
                None,
                query,
            )
            .await
            {
                Ok(data) => data,
                Err(e) if e.contains("404") => {
                    return Ok(session_not_found_error("Get Agent Log", &target_session_id))
                }
                Err(e) => return Err(e),
            };

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

            let data = call_json(
                Method::GET,
                &format!("/api/sessions/{}/children", parent_session_id),
                None,
                None,
            )
            .await?;

            let child_ids = data
                .get("children")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>();

            let mut session_responses: Vec<AgentSessionResponse> = Vec::new();

            for child_id in &child_ids {
                // Fetch each child session data to map it properly
                if let Ok(child_data) = call_json(
                    Method::GET,
                    &format!("/api/sessions/{}", child_id),
                    None,
                    None,
                )
                .await
                {
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
                    "\n\nNo direct sub-agents online. Next step: spawnAgent to deploy a worker.",
                );
            } else {
                message.push_str("\n\nDirect unit roster:\n");
                for resp in &session_responses {
                    message.push_str(&format!(
                        "- {} (ID: {}) status={}\n",
                        resp.name, resp.id, resp.status
                    ));
                    if let Some(summary) = &resp.latest_result {
                        message.push_str(&format!("  latest assistant: {}\n", summary));
                    }
                }
            }

            Ok(success_result(message, session_responses))
        }
        "listAgentTypes" => {
            let data = call_json(Method::GET, "/api/assistants", None, None).await?;
            let assistants = data
                .as_array()
                .cloned()
                .or_else(|| data.get("assistants").and_then(|v| v.as_array()).cloned())
                .or_else(|| data.get("items").and_then(|v| v.as_array()).cloned())
                .unwrap_or_default();

            let message = if assistants.is_empty() {
                "No assistant types available.".to_string()
            } else {
                let mut lines = vec![format!("Available assistant types ({}):", assistants.len())];

                for assistant in &assistants {
                    let id = assistant
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let name = assistant
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed");

                    let config = assistant.get("config").cloned().unwrap_or(json!({}));
                    let parsed_config = if let Some(s) = config.as_str() {
                        serde_json::from_str::<Value>(s).unwrap_or(json!({}))
                    } else {
                        config
                    };
                    let description = extract_assistant_description(&parsed_config);
                    let model = parsed_config
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");

                    lines.push(format!(
                        "- {} [ID: {}]\n  model: {}\n  description: {}",
                        name, id, model, description
                    ));
                }

                lines.join("\n")
            };

            Ok(success_result(message, data))
        }
        "getAgentConfig" => {
            let assistant_id = read_required_string(&args, "assistantId")?;
            let data = call_json(
                Method::GET,
                &format!("/api/assistants/{}", assistant_id),
                None,
                None,
            )
            .await?;

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
