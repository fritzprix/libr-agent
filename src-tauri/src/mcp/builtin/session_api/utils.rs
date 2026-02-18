use serde_json::Value;
use std::collections::{VecDeque, HashSet};

use crate::mcp::types::{MCPResult, MCPContent};
use super::types::MessageSummaryOptions;
use super::formatting::truncate_text;
use crate::repositories::{MessageRepository, SessionRepository};

pub const SWARM_CONTEXT_PREVIEW_LIMIT: usize = 20;
pub const SWARM_MESSAGE_PREVIEW_MAX_CHARS: usize = 140;

pub fn success_result(text: String, data: Value) -> MCPResult {
    MCPResult {
        content: Some(vec![MCPContent::Text {
            text,
            is_error: None,
        }]),
        structured_content: Some(data),
        is_error: Some(false),
    }
}

pub fn read_required_string(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

pub fn resolve_parent_session_id(
    provided_parent: Option<&str>,
    caller_session_id: Option<&str>,
) -> Result<Option<String>, String> {
    let normalized = provided_parent
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match caller_session_id {
        Some(caller_id) => Ok(Some(caller_id.to_string())),
        None => match normalized {
            None => Ok(None),
            Some(value) if value.eq_ignore_ascii_case("current") => Err(
                "parentSessionId='current' requires caller session context. Provide an explicit parentSessionId or call from within a session.".to_string(),
            ),
            Some(value) => Ok(Some(value.to_string())),
        },
    }
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
                latest_assistant_preview_for_session(
                    &child_id,
                    SWARM_MESSAGE_PREVIEW_MAX_CHARS,
                )
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
