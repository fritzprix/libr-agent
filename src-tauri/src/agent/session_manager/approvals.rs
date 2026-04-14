use super::AgentSessionManager;

pub async fn respond_tool_approval(
    manager: &AgentSessionManager,
    session_id: &str,
    tool_call_id: &str,
    approved: bool,
) -> Result<(), String> {
    let active = manager.active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        let mut approvals = session.pending_approvals.write().await;
        if let Some(data) = approvals.remove(tool_call_id) {
            let _ = data.sender.send(approved);
            let event = crate::agent::events::AgentEvent::ToolExecutionApprovalResolved {
                session_id: session_id.to_string(),
                tool_call_id: tool_call_id.to_string(),
                approved,
            };
            if let Err(error) =
                crate::agent::tauri_events::emit_agent_event(&manager.app_handle, event)
            {
                log::error!(
                    "Failed to emit ToolExecutionApprovalResolved event: {}",
                    error
                );
            }
            return Ok(());
        }
    }

    Err(format!(
        "Pending approval not found for tool call: {}",
        tool_call_id
    ))
}

pub async fn respond_channel_permission(
    manager: &AgentSessionManager,
    session_id: &str,
    request_id: &str,
    approved: bool,
) -> Result<String, String> {
    let active = manager.active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Err(format!("Session not found: {}", session_id));
    };

    let matching_tool_call_id = {
        let approvals = session.pending_approvals.read().await;
        crate::agent::tool_approvals::find_pending_approval_tool_call_id(&approvals, request_id)
    };

    drop(active);

    let tool_call_id = matching_tool_call_id.ok_or_else(|| {
        format!(
            "Pending approval not found for request_id: {} in session {}",
            request_id, session_id
        )
    })?;

    respond_tool_approval(manager, session_id, &tool_call_id, approved).await?;

    Ok(tool_call_id)
}
