use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use super::client::call_json;
use super::formatting::{extract_session_status, is_terminal_status, truncate_text};
use super::types::MessageSummaryOptions;
use crate::mcp::types::{MCPContent, MCPResult};
use crate::repositories::{MessageRepository, SessionRepository};

pub const SWARM_CONTEXT_PREVIEW_LIMIT: usize = 20;
pub const SWARM_MESSAGE_PREVIEW_MAX_CHARS: usize = 140;

pub fn success_result<T: serde::Serialize>(text: String, data: T) -> MCPResult {
    MCPResult {
        content: Some(vec![MCPContent::Text {
            text,
            is_error: Some(false),
        }]),
        structured_content: Some(serde_json::to_value(data).unwrap_or(Value::Null)),
        is_error: Some(false),
    }
}

pub fn swarm_error(
    category: crate::mcp::builtin::error_guidance::ErrorCategory,
    operation: &str,
    cause: String,
    hints: Vec<String>,
) -> MCPResult {
    crate::mcp::builtin::error_guidance::guided_error(
        category,
        format!("[{}] {}", operation, cause),
        crate::mcp::builtin::error_guidance::ToolGroup::Agent,
    )
    .guidance(hints)
    .to_mcp_result()
}

pub fn session_not_found_error(operation: &str, session_id: &str) -> MCPResult {
    swarm_error(
        crate::mcp::builtin::error_guidance::ErrorCategory::ResourceNotFound,
        operation,
        format!("Agent session '{}' not found", session_id),
        vec![
            "Use list(type=\"sessions\") to list active delegated sessions".to_string(),
            "Verify the session ID matches one of the active delegated session IDs".to_string(),
            "The session may have been terminated or expired".to_string(),
        ],
    )
}

pub fn read_required_string(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

pub fn read_message_summary_options(args: &Value) -> MessageSummaryOptions {
    let summary_only = args
        .get("summaryOnly")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_raw_preview = args
        .get("includeRawPreview")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let preview_limit = args
        .get("previewLimit")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, 10) as usize)
        .unwrap_or(3);
    let skip_if_unchanged = args
        .get("skipIfUnchanged")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let min_interval_seconds = args
        .get("minIntervalSeconds")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(120))
        .unwrap_or(5);
    let forced_rest_seconds = args
        .get("forcedRestSeconds")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(300))
        .unwrap_or(20);
    let rapid_call_threshold = args
        .get("rapidCallThreshold")
        .and_then(|v| v.as_u64())
        .map(|v| v.clamp(2, 10) as u32)
        .unwrap_or(3);

    MessageSummaryOptions {
        summary_only,
        include_raw_preview,
        preview_limit,
        skip_if_unchanged,
        min_interval_seconds,
        forced_rest_seconds,
        rapid_call_threshold,
    }
}

pub async fn latest_assistant_preview_for_session(
    session_id: &str,
    max_chars: usize,
) -> Option<String> {
    let repo = crate::state::get_message_repository();
    let messages = repo.get_messages_by_session(session_id, 10).await.ok()?;

    for message in messages {
        if message.role != "assistant" {
            continue;
        }

        for item in message.content {
            if let MCPContent::Text { text, .. } = item {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(truncate_text(trimmed, max_chars));
                }
            }
        }

        return Some("[assistant message has no text content]".to_string());
    }

    None
}

pub async fn collect_descendant_snapshot(
    root_session_id: &str,
    max_nodes: usize,
) -> Result<(Vec<(String, String, String, usize, Option<String>)>, bool), String> {
    let repo = crate::state::get_session_repository();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut rows: Vec<(String, String, String, usize, Option<String>)> = Vec::new();
    let mut truncated = false;

    queue.push_back((root_session_id.to_string(), 0));
    visited.insert(root_session_id.to_string());

    'bfs: while let Some((parent_id, parent_depth)) = queue.pop_front() {
        let child_ids = repo
            .get_child_session_ids(&parent_id)
            .await
            .map_err(|e| format!("Failed to fetch child sessions for {}: {}", parent_id, e))?;

        for child_id in child_ids {
            if !visited.insert(child_id.clone()) {
                continue;
            }

            let (name, status) = match repo.get_session(&child_id).await {
                Ok(Some(child)) => (
                    child.name.unwrap_or_else(|| "Unnamed".to_string()),
                    child.status.as_str().to_string(),
                ),
                Ok(None) => ("Unknown".to_string(), "unknown".to_string()),
                Err(_) => ("Unknown".to_string(), "unknown".to_string()),
            };

            let preview = if rows.len() < SWARM_CONTEXT_PREVIEW_LIMIT {
                latest_assistant_preview_for_session(&child_id, SWARM_MESSAGE_PREVIEW_MAX_CHARS)
                    .await
            } else {
                None
            };

            rows.push((child_id.clone(), name, status, parent_depth + 1, preview));

            if rows.len() >= max_nodes {
                truncated = true;
                break 'bfs;
            }

            queue.push_back((child_id, parent_depth + 1));
        }
    }

    Ok((rows, truncated))
}

pub async fn count_session_turns(session_id: &str) -> usize {
    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, 1000)
        .await
        .unwrap_or_default();
    messages.iter().filter(|m| m.role == "assistant").count()
}

/// Consolidates timeout handling for spawnAgent and awaitAgent.
/// Converts Timeout errors into successful MCP results with guidance.
pub fn handle_wait_timeout_result(
    wait_result: Result<(Value, u64), String>,
    session_id: &str,
    timeout_seconds: u64,
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
                let text = if is_spawn {
                    format!(
                        "Child session created (ID: {}) but waiting for completion timed out after {}s.\n\nThe agent is likely still working. Use checkSession(sessionId=\"{}\", wait=true) later to fetch the final result.",
                        session_id, timeout_seconds, session_id
                    )
                } else {
                    format!(
                        "Waiting for session {} timed out after {}s. The agent is likely still working.\n\nYou can call checkSession(sessionId=\"{}\", wait=true) again to continue waiting, or use list(type=\"sessions\") to confirm it is still active.",
                        session_id, timeout_seconds, session_id
                    )
                };

                let data = if is_spawn {
                    serde_json::json!({ "id": session_id, "timeout": true, "error": e })
                } else {
                    serde_json::json!({ "sessionId": session_id, "timeout": true, "error": e })
                };

                return Err(Ok(success_result(text, data)));
            }
            Err(Err(e))
        }
    }
}

pub async fn wait_until_session_terminal(
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
                    "awaitAgent interrupted: calling session was cancelled while waiting for '{}'",
                    session_id
                ));
            }
        }

        let session = call_json(
            reqwest::Method::GET,
            &format!("/api/sessions/{}", session_id),
            None,
            None,
        )
        .await?;

        wake_count = wake_count.saturating_add(1);
        if is_terminal_status(&extract_session_status(&session)) {
            return Ok((session, wake_count));
        }

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

        let sleep_cap = remaining.map(|r| r.min(HEARTBEAT)).unwrap_or(HEARTBEAT);

        tokio::select! {
            _ = child_notifier.notified() => {}
            _ = async {
                match &caller_notifier {
                    Some(n) => n.notified().await,
                    None => std::future::pending::<()>().await,
                }
            } => {}
            _ = sleep(sleep_cap) => {}
        }
    }
}
