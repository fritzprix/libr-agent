use super::AgentSessionManager;
use crate::agent::state::PendingApprovalData;
use std::collections::HashMap;

pub(crate) async fn resolve_pending_tool_approval(
    manager: &AgentSessionManager,
    session_id: &str,
    tool_call_id: &str,
    approved: bool,
) -> Result<bool, String> {
    let active = manager.active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Err(format!("Session not found: {}", session_id));
    };

    let mut approvals = session.pending_approvals.write().await;
    let Some(data) = approvals.remove(tool_call_id) else {
        return Ok(false);
    };
    drop(approvals);
    drop(active);

    let _ = data.sender.send(approved);
    let event = crate::agent::events::AgentEvent::ToolExecutionApprovalResolved {
        session_id: session_id.to_string(),
        tool_call_id: tool_call_id.to_string(),
        approved,
    };
    if let Err(error) = crate::agent::tauri_events::emit_agent_event(&manager.app_handle, event) {
        log::error!(
            "Failed to emit ToolExecutionApprovalResolved event: {}",
            error
        );
    }

    Ok(true)
}

pub async fn respond_tool_approval(
    manager: &AgentSessionManager,
    session_id: &str,
    tool_call_id: &str,
    approved: bool,
) -> Result<(), String> {
    if resolve_pending_tool_approval(manager, session_id, tool_call_id, approved).await? {
        return Ok(());
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

pub async fn approve_all_pending_tool_approvals(
    manager: &AgentSessionManager,
    session_id: &str,
    include_hard_approvals: bool,
) -> Result<usize, String> {
    let active = manager.active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Err(format!("Session not found: {}", session_id));
    };

    let tool_call_ids = {
        let approvals = session.pending_approvals.read().await;
        pending_approval_ids(&approvals, include_hard_approvals)
    };
    drop(active);

    let mut resolved_count = 0usize;
    for tool_call_id in tool_call_ids {
        if resolve_pending_tool_approval(manager, session_id, &tool_call_id, true).await? {
            resolved_count += 1;
        }
    }

    Ok(resolved_count)
}

fn pending_approval_ids(
    approvals: &HashMap<String, PendingApprovalData>,
    include_hard_approvals: bool,
) -> Vec<String> {
    approvals
        .iter()
        .filter(|(_, approval)| {
            include_hard_approvals
                || crate::agent::tool_approvals::pending_approval_is_auto_approvable_in_yolo(
                    approval.approval_kind,
                )
        })
        .map(|(tool_call_id, _)| tool_call_id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::pending_approval_ids;
    use crate::agent::state::{PendingApprovalData, PendingApprovalKind};
    use std::collections::HashMap;
    use tokio::sync::oneshot;

    #[test]
    fn filters_out_hard_approvals_from_yolo_auto_approval() {
        let (standard_tx, _standard_rx) = oneshot::channel();
        let (hard_tx, _hard_rx) = oneshot::channel();

        let mut approvals = HashMap::new();
        approvals.insert(
            "soft-tool".to_string(),
            PendingApprovalData {
                sender: standard_tx,
                tool_name: "runShell".to_string(),
                arguments: "{}".to_string(),
                approval_kind: PendingApprovalKind::Standard,
                request_id: None,
                description: None,
                input_preview: None,
            },
        );
        approvals.insert(
            "hard-tool".to_string(),
            PendingApprovalData {
                sender: hard_tx,
                tool_name: "runShell".to_string(),
                arguments: "{}".to_string(),
                approval_kind: PendingApprovalKind::Hard,
                request_id: None,
                description: None,
                input_preview: None,
            },
        );

        let approved_ids = pending_approval_ids(&approvals, false);
        assert_eq!(approved_ids, vec!["soft-tool".to_string()]);
    }

    #[test]
    fn unsafe_mode_auto_approves_hard_approvals_too() {
        let (standard_tx, _standard_rx) = oneshot::channel();
        let (hard_tx, _hard_rx) = oneshot::channel();

        let mut approvals = HashMap::new();
        approvals.insert(
            "soft-tool".to_string(),
            PendingApprovalData {
                sender: standard_tx,
                tool_name: "runShell".to_string(),
                arguments: "{}".to_string(),
                approval_kind: PendingApprovalKind::Standard,
                request_id: None,
                description: None,
                input_preview: None,
            },
        );
        approvals.insert(
            "hard-tool".to_string(),
            PendingApprovalData {
                sender: hard_tx,
                tool_name: "runShell".to_string(),
                arguments: "{}".to_string(),
                approval_kind: PendingApprovalKind::Hard,
                request_id: None,
                description: None,
                input_preview: None,
            },
        );

        let approved_ids = pending_approval_ids(&approvals, true);
        assert_eq!(approved_ids.len(), 2);
    }
}
