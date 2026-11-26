//! Recovery helpers used when a completion ends in malformed tool markup or
//! repeats a streaming recovery.

use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// A final turn that is only text-formatted tool-call markup for a registered
/// tool, with nothing else queued: the message is discarded (it was not cached
/// or persisted yet) and a corrective retry is started. Returns `true` when a
/// retry was started; `false` leaves the turn to end as it does today.
pub(super) async fn recover_malformed_tool_markup(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    assistant_message: &Message,
) -> Result<bool, String> {
    let text = assistant_message
        .content
        .iter()
        .filter_map(|content| match content {
            crate::mcp::types::MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let Some(tool_name) =
        crate::agent::llm::malformed_tool_markup::malformed_tool_markup_name(&text)
    else {
        return Ok(false);
    };
    if crate::agent::workflow::session_has_pending_events(active_sessions, session_id).await {
        return Ok(false);
    }
    let registered = crate::agent::tools::collect_available_tools(session_id, proxy_manager)
        .await
        .map(|tools| tools.iter().any(|tool| tool.name == tool_name))
        .unwrap_or(false);
    if !registered {
        return Ok(false);
    }

    let outcome = crate::agent::llm::stream_recovery::handle_malformed_tool_markup_completion(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id.to_string(),
        assistant_message.id.clone(),
        tool_name,
    )
    .await?;
    Ok(matches!(
        outcome,
        crate::agent::llm::stream_recovery::StreamingIssueOutcome::Retried { .. }
    ))
}

pub(super) async fn reset_streaming_recovery_retry_counts(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        *session.repeated_thinking_retry_count.write().await = 0;
        *session.repeated_text_loop_retry_count.write().await = 0;
        *session.reasoning_budget_retry_count.write().await = 0;
        *session.malformed_tool_markup_retry_count.write().await = 0;
        *session.invalid_tool_args_retry_count.write().await = 0;
    }
}
