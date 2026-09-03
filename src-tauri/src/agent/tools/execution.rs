use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::classification::{classify_tool_result, ToolResultAcceptance};
use super::media::externalize_media_content_for_storage;
use super::messages::{
    add_cancellation_metadata, append_cancellation_note, append_cancellation_note_to_error,
    create_error_tool_result, create_tool_result_message, create_tool_result_message_with_content,
};

/// Handle tool execution result from frontend or internal execution
///
/// Returns `Ok(Some(messages))` if all pending tools for this turn have completed,
/// containing the accumulated tool results to be processed.
/// Returns `Ok(None)` if we are still waiting for other tools to complete.
pub async fn handle_tool_result(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    tool_call_id: String,
    result: crate::commands::agent_commands::ToolExecutionResult,
) -> Result<Option<(Message, bool)>, String> {
    let crate::commands::agent_commands::ToolExecutionResult {
        success,
        content,
        mcp_content,
        structured_content,
        error,
        is_error,
        cancellation,
    } = result;
    let mcp_content = match mcp_content {
        Some(content) => Some(externalize_media_content_for_storage(&session_id, content).await?),
        None => None,
    };
    let structured_content = add_cancellation_metadata(structured_content, cancellation.as_ref());

    log::debug!(
        "Tool result received for session {}, tool_call_id: {}",
        session_id,
        tool_call_id
    );

    // Scope to hold the write lock
    {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            if let Some(pending) = &mut session.pending_execution {
                match classify_tool_result(pending, &tool_call_id) {
                    ToolResultAcceptance::Stale => {
                        log::warn!(
                            "Ignoring stale tool result for session {}: tool_call_id {} does not belong to message {}",
                            session_id,
                            tool_call_id,
                            pending.message_id
                        );
                        return Ok(None);
                    }
                    ToolResultAcceptance::Duplicate => {
                        log::warn!(
                            "Ignoring duplicate tool result for session {}: tool_call_id {} already handled for message {}",
                            session_id,
                            tool_call_id,
                            pending.message_id
                        );
                        return Ok(None);
                    }
                    ToolResultAcceptance::Accept => {}
                }

                // Create Tool Message using helper methods
                let message = if is_error {
                    if let Some(mut mcp_content) = mcp_content {
                        append_cancellation_note(&mut mcp_content, cancellation.as_ref());
                        // Prefer structured content (guided_error) over bare error string —
                        // the content array carries the full diagnosis the agent needs.
                        create_tool_result_message_with_content(
                            &session_id,
                            &tool_call_id,
                            mcp_content,
                            structured_content.clone(),
                            true,
                        )
                    } else {
                        let error_message = append_cancellation_note_to_error(
                            error
                                .as_deref()
                                .unwrap_or("Tool execution failed without error message"),
                            cancellation.as_ref(),
                        );
                        create_error_tool_result(
                            &session_id,
                            &tool_call_id,
                            &error_message,
                            structured_content.clone(),
                        )
                    }
                } else if let Some(mcp_content) = mcp_content {
                    // ✅ ALWAYS use structured content for successful tool calls
                    create_tool_result_message_with_content(
                        &session_id,
                        &tool_call_id,
                        mcp_content,
                        structured_content.clone(),
                        false,
                    )
                } else {
                    // ⚠️ This branch should never happen for successful tool calls
                    log::warn!(
                        "Tool result has no mcp_content for session {}, tool_call_id {}. Using stringified fallback.",
                        session_id,
                        tool_call_id
                    );
                    create_tool_result_message(
                        &session_id,
                        &tool_call_id,
                        content.clone(),
                        structured_content.clone(),
                    )
                };

                pending.completed_tool_call_ids.insert(tool_call_id.clone());

                // Emit ToolExecutionCompleted event for external tools (progress tracking)
                if let Some(tool_name) = pending.tool_names.get(&tool_call_id) {
                    let event = crate::agent::events::AgentEvent::ToolExecutionCompleted {
                        session_id: session_id.clone(),
                        tool_name: tool_name.clone(),
                        success: !is_error && success,
                    };
                    let _ = crate::agent::tauri_events::emit_agent_event(app_handle, event);
                }

                log::debug!(
                    "Accumulated result {}/{} for session {}",
                    pending.completed_tool_call_ids.len(),
                    pending.total_expected,
                    session_id
                );

                // Check if all results are in
                let all_completed = pending.completed_tool_call_ids.len() >= pending.total_expected;
                if all_completed {
                    // Clear pending state
                    session.pending_execution = None;
                }

                Ok(Some((message, all_completed)))
            } else {
                log::warn!(
                    "Received tool result for session {} but no pending execution state found",
                    session_id
                );
                Ok(None) // Ignore or error? Safe to ignore to prevent crashes
            }
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }
}
