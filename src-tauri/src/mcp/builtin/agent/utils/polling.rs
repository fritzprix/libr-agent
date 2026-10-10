use super::extraction::format_message_summary;
use super::payload::{build_agent_session_tool_data, check_session_next_actions};
use super::query::{count_session_turns, fetch_session_value};
use crate::agent::AgentSessionManager;
use crate::mcp::types::{MCPContent, MCPResult};
use crate::repositories::MessageRepository;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub fn extract_session_status(session: &Value) -> String {
    session
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "idle" | "terminated" | "failed" | "error"
    )
}

pub fn is_wait_complete_status(status: &str) -> bool {
    is_terminal_status(status) || status.eq_ignore_ascii_case("paused")
}

/// When a blocking wait times out, promote to success if the child already
/// settled. Avoids false timeouts when Idle races the deadline or a
/// `Notify::notify_waiters` wake is lost between status read and `select!`.
pub fn should_promote_wait_timeout_to_terminal(status: &str) -> bool {
    is_wait_complete_status(status)
}

/// Map a freshly fetched session JSON into a wait-success tuple when the child
/// already settled. Used by [`handle_wait_timeout_result`] after a timeout error
/// (and covered by unit tests without a live [`AgentSessionManager`]).
///
/// The `u64` is a loop-wake placeholder (`1`); callers currently discard it.
pub fn promote_settled_session_after_wait_timeout(session: Option<Value>) -> Option<(Value, u64)> {
    let session = session?;
    let status = extract_session_status(&session);
    if should_promote_wait_timeout_to_terminal(&status) {
        Some((session, 1))
    } else {
        None
    }
}

pub async fn wait_until_session_terminal(
    manager: &AgentSessionManager,
    session_id: &str,
    timeout_seconds: u64,
    caller_session_id: Option<&str>,
) -> Result<(Value, u64), String> {
    const HEARTBEAT: Duration = Duration::from_secs(30);

    let bus = crate::state::get_session_bus();
    let child_notifier = bus.get_or_create(session_id);
    let caller_notifier = caller_session_id.map(|id| bus.get_or_create(id));
    let caller_cancel_pending: Option<Arc<std::sync::atomic::AtomicBool>> = match caller_session_id
    {
        Some(id) => crate::state::get_session_cancel_pending(id).await,
        None => None,
    };

    let started_at = Instant::now();
    let mut wake_count: u64 = 0;

    loop {
        if let Some(ref flag) = caller_cancel_pending {
            if flag.load(Ordering::Relaxed) {
                return Err(format!(
                    "agent__checkSession interrupted: calling session was cancelled while waiting for '{}'",
                    session_id
                ));
            }
        }

        // Subscribe before the status read so a notify_one permit issued while
        // we fetch is not lost. notify_waiters alone still requires an active
        // waiter; session_bus also calls notify_one for late registrants.
        let child_notified = child_notifier.notified();
        let caller_wait = async {
            if let Some(ref notifier) = caller_notifier {
                notifier.notified().await;
            } else {
                futures::future::pending::<()>().await;
            }
        };
        tokio::pin!(caller_wait);

        let session = fetch_session_value(manager, session_id)
            .await?
            .ok_or_else(|| format!("Agent session '{}' not found", session_id))?;

        wake_count = wake_count.saturating_add(1);
        if is_wait_complete_status(&extract_session_status(&session)) {
            return Ok((session, wake_count));
        }

        let remaining = if timeout_seconds == 0 {
            None
        } else {
            let limit = Duration::from_secs(timeout_seconds.clamp(5, 86_400));
            let elapsed = started_at.elapsed();
            if elapsed >= limit {
                // Deadline hit: re-read once so Idle-during-wait is not a false timeout.
                if let Some(session) = fetch_session_value(manager, session_id).await? {
                    wake_count = wake_count.saturating_add(1);
                    let status = extract_session_status(&session);
                    if is_wait_complete_status(&status) {
                        log::info!(
                            "checkSession wait deadline recovered settled session {}: status={}, wake_count={}",
                            session_id,
                            status,
                            wake_count
                        );
                        return Ok((session, wake_count));
                    }
                }
                return Err(format!(
                    "agent__checkSession timed out after {}s for session {}",
                    timeout_seconds, session_id
                ));
            }
            Some(limit - elapsed)
        };

        let sleep_cap = remaining.map(|r| r.min(HEARTBEAT)).unwrap_or(HEARTBEAT);

        tokio::select! {
            _ = sleep(sleep_cap) => {}
            _ = child_notified => {}
            _ = &mut caller_wait => {}
        }
    }
}

pub async fn handle_wait_timeout_result(
    wait_result: Result<(Value, u64), String>,
    manager: Option<&AgentSessionManager>,
    session_id: &str,
    timeout_seconds: u64,
    tool_name: &str,
    is_spawn: bool,
) -> Result<(Value, u64), Result<MCPResult, String>> {
    match wait_result {
        Ok(res) => Ok(res),
        Err(e) => {
            let (category, _) = crate::mcp::error_normalization::categorize_session_api_error(&e);
            if matches!(
                category,
                crate::mcp::error_normalization::ExternalMcpErrorCategory::Timeout
            ) {
                // Single fetch: promote if settled, else reuse status for timeout text.
                let fetched_session = match manager {
                    Some(manager) => fetch_session_value(manager, session_id)
                        .await
                        .ok()
                        .flatten(),
                    None => None,
                };

                if let Some((session, wake_count)) =
                    promote_settled_session_after_wait_timeout(fetched_session.clone())
                {
                    log::info!(
                        "Promoting checkSession wait timeout to terminal for session {}: status={}",
                        session_id,
                        extract_session_status(&session)
                    );
                    return Ok((session, wake_count));
                }

                let (session_status, turn_count, latest_msgs_str, latest_msgs_json) = match manager
                {
                    Some(_) => {
                        let session_status = fetched_session
                            .as_ref()
                            .map(extract_session_status)
                            .unwrap_or_else(|| "unknown".to_string());
                        let turn_count = count_session_turns(session_id).await;

                        let repo = crate::state::get_message_repository();
                        let mut messages = repo
                            .get_messages_by_session(session_id, 5)
                            .await
                            .unwrap_or_default();
                        messages.reverse();

                        let mut msgs_str = String::new();
                        let mut msgs_json = Vec::new();

                        if !messages.is_empty() {
                            msgs_str.push_str("\n\nLatest workflow messages:\n");
                            for msg in &messages {
                                let summary = format_message_summary(msg);
                                msgs_str.push_str(&format!("  - [{}]: {}\n", msg.role, summary));
                                msgs_json.push(json!({
                                    "role": msg.role,
                                    "summary": summary,
                                    "createdAt": msg.created_at,
                                }));
                            }
                        }

                        (session_status, turn_count, msgs_str, msgs_json)
                    }
                    None => ("unknown".to_string(), 0, String::new(), Vec::new()),
                };

                let display_id = crate::utils::session_id::display_session_id(session_id);
                let text = if is_spawn {
                    format!(
                        "Child session created (ID: {}) but waiting for completion timed out after {}s.\n\nThe agent is likely still working. Use agent__checkSession(sessionId=\"{}\", wait=true) later to fetch the final result.{}\n\nCurrent status: {}",
                        display_id, timeout_seconds, display_id, latest_msgs_str, session_status
                    )
                } else {
                    format!(
                        "Waiting for session {} timed out after {}s. The agent is likely still working.\n\nYou can call agent__checkSession(sessionId=\"{}\", wait=true) again to continue waiting, or use agent__listAgents(type=\"sessions\") to confirm it is still active.{}\n\nCurrent status: {}",
                        display_id, timeout_seconds, display_id, latest_msgs_str, session_status
                    )
                };

                let mut data = build_agent_session_tool_data(
                    tool_name,
                    session_id,
                    &text,
                    &session_status,
                    "timeout",
                    turn_count,
                    check_session_next_actions(session_id),
                );
                data.insert("timeout".to_string(), json!(true));
                data.insert("timeoutSeconds".to_string(), json!(timeout_seconds));
                data.insert(
                    "errorCategory".to_string(),
                    Value::String("timeout".to_string()),
                );
                data.insert("error".to_string(), Value::String(e));
                data.insert("latestMessages".to_string(), json!(latest_msgs_json));

                if is_spawn {
                    data.insert("id".to_string(), Value::String(display_id));
                }

                Err(Ok(MCPResult {
                    content: Some(vec![MCPContent::Text { text }]),
                    structured_content: Some(Value::Object(data)),
                    is_error: Some(false),
                }))
            } else {
                Err(Err(e))
            }
        }
    }
}
