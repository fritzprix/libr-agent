use reqwest::Method;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::mcp::types::MCPResult;
use crate::repositories::SessionRepository;

use super::client::call_json;
use super::formatting::*;
use super::utils::*;
use super::cache::*;

async fn wait_until_session_terminal(
    session_id: &str,
    timeout_seconds: u64,
    poll_interval_seconds: u64,
) -> Result<(Value, u64), String> {
    let timeout_seconds = timeout_seconds.clamp(5, 900);
    let poll_interval_seconds = poll_interval_seconds.clamp(1, 30);

    let started_at = Instant::now();
    let mut poll_count: u64 = 0;

    loop {
        let session = call_json(
            Method::GET,
            &format!("/api/sessions/{}", session_id),
            None,
            None,
        )
        .await?;

        poll_count = poll_count.saturating_add(1);
        let status = extract_session_status(&session);
        if is_terminal_status(&status) {
            return Ok((session, poll_count));
        }

        if started_at.elapsed() >= Duration::from_secs(timeout_seconds) {
            return Err(format!(
                "waitForSessionIdle timed out after {}s for session {}",
                timeout_seconds, session_id
            ));
        }

        sleep(Duration::from_secs(poll_interval_seconds)).await;
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
        "createSession" => {
            let assistant_id = read_required_string(&args, "assistantId")?;
            let request = read_required_string(&args, "request")?;

            let mut body = json!({
                "assistantId": assistant_id,
                "request": request,
            });

            // Parent is optional - auto-attach from caller context if available
            let parent_session_id = resolve_parent_session_id(
                args.get("parentSessionId").and_then(|v| v.as_str()),
                caller_session_id.as_deref(),
            )?;

            if let Some(pid) = &parent_session_id {
                body["parentSessionId"] = Value::String(pid.clone());
            }

            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                body["name"] = Value::String(name.to_string());
            }

            if let Some(path) = args.get("workspacePath").and_then(|v| v.as_str()) {
                body["workspacePath"] = Value::String(path.to_string());
            }

            if let Some(max_depth) = args.get("maxDepth").and_then(|v| v.as_u64()) {
                body["maxDepth"] = Value::Number(max_depth.into());
            }

            if let Some(max_fanout) = args.get("maxFanout").and_then(|v| v.as_u64()) {
                body["maxFanout"] = Value::Number(max_fanout.into());
            }

            let data = call_json(Method::POST, "/api/sessions", Some(body), None).await?;

            let session_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let depth = data.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);

            let parent_info = parent_session_id
                .as_deref()
                .map(|pid| format!(", parent: {}", pid))
                .unwrap_or_default();

            Ok(success_result(
                format!(
                    "Session created: {} (depth: {}{})\nUse getMessages(\"{}\") to poll progress.",
                    session_id, depth, parent_info, session_id
                ),
                data,
            ))
        }
        "createChildSession" => {
            let assistant_id = read_required_string(&args, "assistantId")?;
            let request = read_required_string(&args, "request")?;

            let parent_session_id = resolve_parent_session_id(
                args.get("parentSessionId").and_then(|v| v.as_str()),
                caller_session_id.as_deref(),
            )?
            .ok_or_else(|| {
                "Missing parent session context: provide explicit parentSessionId or call from within a parent session"
                    .to_string()
            })?;

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

            if let Some(max_depth) = args.get("maxDepth").and_then(|v| v.as_u64()) {
                body["maxDepth"] = Value::Number(max_depth.into());
            }

            if let Some(max_fanout) = args.get("maxFanout").and_then(|v| v.as_u64()) {
                body["maxFanout"] = Value::Number(max_fanout.into());
            }

            let data = call_json(Method::POST, "/api/sessions", Some(body), None).await?;

            let child_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let depth = data.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
            let lineage = data
                .get("lineageId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            Ok(success_result(
                format!(
                    "Child session created: {} (parent: {}, depth: {}, lineage: {})",
                    child_id, parent_session_id, depth, lineage
                ),
                data,
            ))
        }
        "getSession" => {
            let session_id = read_required_string(&args, "sessionId")?;
            let data = call_json(
                Method::GET,
                &format!("/api/sessions/{}", session_id),
                None,
                None,
            )
            .await?;
            Ok(success_result(
                format!("Fetched session: {}", session_id),
                data,
            ))
        }
        "waitForSessionIdle" => {
            let session_id = read_required_string(&args, "sessionId")?;

            let timeout_seconds = args
                .get("timeoutSeconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(180);
            let poll_interval_seconds = args
                .get("pollIntervalSeconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(3);
            let include_last_assistant_message = args
                .get("includeLastAssistantMessage")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
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

            let (session_data, poll_count) = wait_until_session_terminal(
                &session_id,
                timeout_seconds,
                poll_interval_seconds,
            )
            .await?;

            let final_status = extract_session_status(&session_data);

            if !include_last_assistant_message {
                return Ok(success_result(
                    format!(
                        "Session {} reached terminal status '{}' after {} polls.",
                        session_id, final_status, poll_count
                    ),
                    json!({
                        "session": session_data,
                        "status": final_status,
                        "pollCount": poll_count,
                        "messages": Value::Null
                    }),
                ));
            }

            let messages_data = call_json(
                Method::GET,
                &format!("/api/sessions/{}/messages", session_id),
                None,
                Some(vec![("limit".to_string(), result_message_limit.to_string())]),
            )
            .await?;

            let messages = messages_data
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let text = if let Some((message_id, assistant_text)) =
                latest_assistant_message_text(&messages, assistant_message_max_chars)
            {
                format!(
                    "Session {} reached terminal status '{}' after {} polls.\n\nLatest assistant result [{}]:\n{}",
                    session_id, final_status, poll_count, message_id, assistant_text
                )
            } else {
                format!(
                    "Session {} reached terminal status '{}' after {} polls.\n\nNo assistant text message was found in the latest {} messages.",
                    session_id, final_status, poll_count, result_message_limit
                )
            };

            Ok(success_result(
                text,
                json!({
                    "session": session_data,
                    "status": final_status,
                    "pollCount": poll_count,
                    "messages": messages_data
                }),
            ))
        }
        "getMessages" => {
            let target_session_id = read_required_string(&args, "sessionId")?;

            let requested_limit = args.get("limit").and_then(|v| v.as_u64());
            let options = read_message_summary_options(&args);

            if options.skip_if_unchanged {
                if let Some(wait_notice) = min_interval_notice(
                    caller_session_id.as_deref(),
                    &target_session_id,
                    requested_limit,
                    options,
                )
                .await
                {
                    return Ok(success_result(
                        wait_notice,
                        json!({
                            "sessionId": target_session_id,
                            "skipped": true,
                            "reason": "min_interval",
                            "minIntervalSeconds": options.min_interval_seconds,
                            "forcedRestSeconds": options.forced_rest_seconds,
                            "rapidCallThreshold": options.rapid_call_threshold
                        }),
                    ));
                }
            }

            let query = requested_limit.map(|v| vec![("limit".to_string(), v.to_string())]);

            let data = call_json(
                Method::GET,
                &format!("/api/sessions/{}/messages", target_session_id),
                None,
                query,
            )
            .await?;

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

            Ok(success_result(summary_text, data))
        }
        "getChildSessions" => {
            let parent_session_id = read_required_string(&args, "parentSessionId")?;

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

            let repo = crate::state::get_session_repository();
            let mut rows: Vec<(String, String, String, Option<String>)> = Vec::new();

            for child_id in &child_ids {
                let (name, status) = match repo.get_session(child_id).await {
                    Ok(Some(child)) => (
                        child.name.unwrap_or_else(|| "Unnamed".to_string()),
                        child.status.as_str().to_string(),
                    ),
                    Ok(None) => ("Unknown".to_string(), "unknown".to_string()),
                    Err(_) => ("Unknown".to_string(), "unknown".to_string()),
                };

                let preview = latest_assistant_preview_for_session(
                    child_id,
                    SWARM_MESSAGE_PREVIEW_MAX_CHARS,
                )
                .await;

                rows.push((child_id.clone(), name, status, preview));
            }

            let mut message = format!(
                "Fetched {} direct sub-agents for commander session {}",
                child_ids.len(),
                parent_session_id
            );

            if rows.is_empty() {
                message.push_str(
                    "\n\nNo direct sub-agents online. Next step: createChildSession to deploy a worker.",
                );
            } else {
                message.push_str("\n\nDirect unit roster:\n");
                for (child_id, name, status, preview) in rows {
                    message.push_str(&format!(
                        "- {} (ID: {}) status={}\n",
                        name, child_id, status
                    ));
                    if let Some(summary) = preview {
                        message.push_str(&format!("  latest assistant: {}\n", summary));
                    }
                }
            }

            Ok(success_result(message, data))
        }
        "sendMessage" => {
            let session_id = read_required_string(&args, "sessionId")?;
            let content = read_required_string(&args, "content")?;

            let data = call_json(
                Method::POST,
                &format!("/api/sessions/{}/messages", session_id),
                Some(json!({ "content": content })),
                None,
            )
            .await?;

            let message_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let status = data
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            Ok(success_result(
                format!("Message accepted: {} (status: {})", message_id, status),
                data,
            ))
        }
        "terminateSession" => {
            let session_id = read_required_string(&args, "sessionId")?;
            let data = call_json(
                Method::POST,
                &format!("/api/sessions/{}/terminate", session_id),
                None,
                None,
            )
            .await?;

            Ok(success_result(
                format!("Terminated session: {}", session_id),
                data,
            ))
        }
        "listAssistants" => {
            let data = call_json(Method::GET, "/api/assistants", None, None).await?;

            // Extract assistant details for text output (AI agents need to see this!)
            let assistants_text = data
                .get("assistants")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| {
                            let name = a.get("name")?.as_str()?;
                            let id = a.get("id")?.as_str()?;

                            // Parse config (might be string or object)
                            let config = a.get("config")?;
                            let parsed_config = if let Some(config_str) = config.as_str() {
                                serde_json::from_str::<Value>(config_str).ok()?
                            } else {
                                config.clone()
                            };

                            // Extract description from config
                            let description = extract_assistant_description(&parsed_config);

                            Some(format!(
                                "• {} [ID: {}]\n  Description: {}",
                                name, id, description
                            ))
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n")
                })
                .unwrap_or_default();

            let assistant_count = data
                .get("assistants")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);

            let text = if assistant_count == 0 {
                "No assistants found".to_string()
            } else {
                format!(
                    "Found {} {}:\n\n{}",
                    assistant_count,
                    if assistant_count == 1 {
                        "assistant"
                    } else {
                        "assistants"
                    },
                    assistants_text
                )
            };

            Ok(success_result(text, data))
        }
        "getAssistant" => {
            let assistant_id = read_required_string(&args, "assistantId")?;
            let data = call_json(
                Method::GET,
                &format!("/api/assistants/{}", assistant_id),
                None,
                None,
            )
            .await?;

            let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
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
