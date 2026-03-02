use reqwest::Method;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::mcp::types::MCPResult;
use crate::repositories::SessionRepository;

use super::cache::*;
use super::client::call_json;
use super::formatting::*;
use super::utils::*;

/// Wait until `session_id` reaches a terminal status.
///
/// # SP6 — Responsive parent cancellation
/// When the Parent calls `cancel_workflow` while blocked here:
/// - The *deferred* cancel path sets `cancel_pending = true` on the parent and
///   calls `SessionBus::notify_status_change` on the parent's own bus entry.
/// - This wakes the select! via the `caller_notifier` branch.
/// - The next loop iteration checks `caller_cancel_pending.load(Relaxed)` and
///   returns an error immediately, without waiting up to 30 s for the heartbeat.
///
/// The caller still calls `gate.resume_agent()` after this returns Err, so the
/// suspended gate slot is always released correctly (no leak).
async fn wait_until_session_terminal(
    session_id: &str,
    timeout_seconds: u64, // 0 = indefinite; otherwise clamped to 5..86400 s
    caller_session_id: Option<&str>, // SP6: id of the parent/calling session
) -> Result<(Value, u64), String> {
    const HEARTBEAT: Duration = Duration::from_secs(30);

    let bus = crate::state::get_session_bus();
    let child_notifier = bus.get_or_create(session_id);
    // Subscribe to the caller's bus so cancel_workflow's notify_status_change wakes us.
    let caller_notifier = caller_session_id.map(|id| bus.get_or_create(id));
    // Clone the cancel_pending Arc once — we can then poll it lock-free in the loop.
    let caller_cancel_pending: Option<Arc<std::sync::atomic::AtomicBool>> = match caller_session_id
    {
        Some(id) => crate::state::get_session_cancel_pending(id).await,
        None => None,
    };

    let started_at = Instant::now();
    let mut wake_count: u64 = 0;

    loop {
        // SP6: check before hitting the HTTP endpoint to short-circuit fast.
        if let Some(ref flag) = caller_cancel_pending {
            if flag.load(Ordering::Relaxed) {
                return Err(format!(
                    "awaitAgent interrupted: calling session was cancelled while waiting for '{}'",
                    session_id
                ));
            }
        }

        // Check current status first — avoids a spurious wait when the session
        // is already terminal by the time we arrive here.
        let session = call_json(
            Method::GET,
            &format!("/api/sessions/{}", session_id),
            None,
            None,
        )
        .await?;

        wake_count = wake_count.saturating_add(1);
        if is_terminal_status(&extract_session_status(&session)) {
            return Ok((session, wake_count));
        }

        // Determine remaining budget (None = indefinite).
        let remaining = if timeout_seconds == 0 {
            None
        } else {
            let limit = Duration::from_secs(timeout_seconds.clamp(5, 86_400));
            let elapsed = started_at.elapsed();
            if elapsed >= limit {
                return Err(format!(
                    "awaitAgent timed out after {}s for session {}",
                    timeout_seconds, session_id
                ));
            }
            Some(limit - elapsed)
        };

        // Sleep until notified by child status change, caller cancel, heartbeat,
        // or hard deadline — whichever comes first.
        let sleep_cap = remaining.map(|r| r.min(HEARTBEAT)).unwrap_or(HEARTBEAT);

        tokio::select! {
            _ = child_notifier.notified() => {} // Child status changed — re-check.
            _ = async {
                match &caller_notifier {
                    Some(n) => n.notified().await,
                    None => std::future::pending::<()>().await,
                }
            } => {} // Caller was notified (cancel or status change) — check flag at loop top.
            _ = sleep(sleep_cap) => {}    // Heartbeat or deadline — re-check.
        }
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

            let await_completion = args
                .get("awaitCompletion")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let timeout_seconds = args
                .get("timeoutSeconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(180);
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

            let data = call_json(Method::POST, "/api/sessions", Some(body), None).await?;

            let child_id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let depth = data.get("depth").and_then(|v| v.as_u64()).unwrap_or(0);
            let lineage = data
                .get("lineageId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            if !await_completion {
                return Ok(success_result(
                    format!(
                        "Child session created: {} (parent: {}, depth: {}, lineage: {})\n\nUse awaitAgent(\"{}\") to wait for completion.",
                        child_id, parent_session_id, depth, lineage, child_id
                    ),
                    data,
                ));
            }

            // awaitCompletion=true: SP2 two-phase transition + SP1 push-notify wait.
            let child_id_owned = child_id.to_string();
            let (session_data, poll_count) = {
                let gate = crate::state::get_concurrency_gate();
                gate.suspend_agent().await?; // active → suspended (frees slot for child)
                let wait_result = wait_until_session_terminal(
                    &child_id_owned,
                    timeout_seconds,
                    caller_session_id.as_deref(), // SP6
                )
                .await;
                gate.resume_agent().await?; // suspended → active (always, even on timeout)
                wait_result?
            };

            let final_status = extract_session_status(&session_data);

            if !include_last_assistant_message {
                return Ok(success_result(
                    format!(
                        "Child session {} (depth: {}, lineage: {}) reached terminal status '{}' after {} polls.",
                        child_id_owned, depth, lineage, final_status, poll_count
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
                &format!("/api/sessions/{}/messages", child_id_owned),
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

            let text = if let Some((message_id, assistant_text)) =
                latest_assistant_message_text(&messages, assistant_message_max_chars)
            {
                format!(
                    "Child session {} (depth: {}, lineage: {}) completed with status '{}' after {} polls.\n\nLatest assistant result [{}]:\n{}",
                    child_id_owned, depth, lineage, final_status, poll_count, message_id, assistant_text
                )
            } else {
                format!(
                    "Child session {} (depth: {}, lineage: {}) completed with status '{}' after {} polls.\n\nNo assistant text message found in the latest {} messages.",
                    child_id_owned, depth, lineage, final_status, poll_count, result_message_limit
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
        "getAgentStatus" => {
            let session_id = read_required_string(&args, "sessionId")?;
            let data = call_json(
                Method::GET,
                &format!("/api/sessions/{}", session_id),
                None,
                None,
            )
            .await?;
            let status = extract_session_status(&data);
            let name = data
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed");
            Ok(success_result(
                format!("Session {} ({}): status={}", name, session_id, status),
                data,
            ))
        }
        "awaitAgent" => {
            let session_id = read_required_string(&args, "sessionId")?;

            let timeout_seconds = args
                .get("timeoutSeconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(180);
            // pollIntervalSeconds is accepted for backward compatibility but ignored;
            // awaitAgent now uses push notifications instead of polling.
            let _poll_interval_seconds = args
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

            // Pre-check: if child is paused (crash recovery), auto-resume unless it's
            // an intentional pause waiting for a UI resource interaction.
            {
                let initial_session = call_json(
                    Method::GET,
                    &format!("/api/sessions/{}", session_id),
                    None,
                    None,
                )
                .await
                .unwrap_or_default();
                if extract_session_status(&initial_session) == "paused" {
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

                    if last_message_is_ui_resource(&msgs) {
                        log::info!(
                            "awaitAgent: session '{}' is paused waiting for UI resource — leaving alone",
                            session_id
                        );
                    } else {
                        log::info!(
                            "awaitAgent: session '{}' is paused from crash, resuming via /resume endpoint",
                            session_id
                        );
                        // Use /resume instead of /messages to avoid injecting a garbage user message.
                        // /resume loads the session into memory and triggers workflow from existing messages.
                        match call_json(
                            Method::POST,
                            &format!("/api/sessions/{}/resume", session_id),
                            None,
                            None,
                        )
                        .await
                        {
                            Ok(_) => {
                                log::info!(
                                    "awaitAgent: resume triggered for '{}', session now busy",
                                    session_id
                                );
                            }
                            Err(e) => {
                                log::warn!("awaitAgent: resume failed for '{}': {}", session_id, e);
                            }
                        }
                        // Brief pause to allow the session to transition to busy
                        sleep(Duration::from_millis(500)).await;
                    }
                }
            }

            let (session_data, poll_count) = {
                // SP2: Two-phase slot transition.
                //
                // The calling (parent) session is currently holding an active-agent slot.
                // Before we block waiting for the child, we swap: acquire a suspended slot
                // and release the active slot so the child can run.  On wakeup we reverse
                // the swap to re-enter the active pool.
                //
                // resume_agent() is called regardless of whether the wait succeeded or
                // timed-out, so the parent always re-acquires its active slot before
                // returning to the LLM loop.
                //
                let gate = crate::state::get_concurrency_gate();
                gate.suspend_agent().await?; // active → suspended
                let wait_result = wait_until_session_terminal(
                    &session_id,
                    timeout_seconds,
                    caller_session_id.as_deref(), // SP6
                )
                .await;
                gate.resume_agent().await?; // suspended → active (always)

                // On timeout the child is still running — terminate it so it
                // doesn't linger as a zombie consuming resources.
                if let Err(ref e) = wait_result {
                    if e.contains("timed out") {
                        log::warn!(
                            "awaitAgent: timeout for session '{}', terminating child",
                            session_id
                        );
                        let _ = call_json(
                            Method::POST,
                            &format!("/api/sessions/{}/terminate", session_id),
                            None,
                            None,
                        )
                        .await;
                    }
                }

                wait_result?
            };

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
        "getAgentLog" => {
            let target_session_id = read_required_string(&args, "sessionId")?;

            let requested_limit = args.get("limit").and_then(|v| v.as_u64());
            let options = read_message_summary_options(&args);

            // Enforce rate-limit delay; returns Some(hint) when rapid-poll
            // threshold was hit, so we can nudge the agent toward awaitAgent.
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

            // Prepend awaitAgent hint if rapid polling was severe enough.
            let final_text = match rapid_poll_hint {
                Some(hint) => format!("{}\n\n---\n\n{}", hint, summary_text),
                None => summary_text,
            };

            Ok(success_result(final_text, data))
        }
        "getChildAgents" => {
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

                let preview =
                    latest_assistant_preview_for_session(child_id, SWARM_MESSAGE_PREVIEW_MAX_CHARS)
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
                    "\n\nNo direct sub-agents online. Next step: spawnAgent to deploy a worker.",
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
        "messageAgent" => {
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
        "terminateAgent" => {
            let session_id = read_required_string(&args, "sessionId")?;

            // Prevent an agent from terminating itself — self-termination mid-execution
            // leaves the session in a torn state (tool call never returns, loop hangs).
            if caller_session_id.as_deref() == Some(session_id.as_str()) {
                return Err(
                    "Self-termination is not allowed. An agent cannot terminate its own session. \
                     Use the planning tool to mark the task complete instead."
                        .to_string(),
                );
            }

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
        "listAgentTypes" => {
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

                            let description = extract_assistant_description(&parsed_config);

                            // Capability hints from enabled MCP server keys
                            // (model is session-level, not stored in assistant config)
                            let capabilities: Vec<&str> = parsed_config
                                .get("mcpServers")
                                .and_then(|v| v.as_object())
                                .map(|obj| obj.keys().map(|k| k.as_str()).collect())
                                .unwrap_or_default();

                            let cap_hint = if capabilities.is_empty() {
                                String::new()
                            } else {
                                format!(" | {}", capabilities.join(", "))
                            };

                            let desc_line = if description == "No description" {
                                String::new()
                            } else {
                                format!("\n  \"{}\"", description)
                            };

                            Some(format!(
                                "• {} [ID: {}]{}{}\n",
                                name, id, cap_hint, desc_line
                            ))
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
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
