use super::AgentSessionManager;
use crate::execution_mode::ExecutionMode;
use std::sync::atomic::Ordering;

pub async fn set_execution_mode(
    manager: &AgentSessionManager,
    session_id: &str,
    mode: ExecutionMode,
) -> Result<(), String> {
    let (yolo_enabled, unsafe_enabled) = mode.runtime_flags();
    manager
        .session_repo
        .update_execution_mode(session_id, mode)
        .await
        .map_err(|e| format!("Failed to update session execution mode: {}", e))?;

    let has_active_session = {
        let active = manager.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            session.yolo_mode.store(yolo_enabled, Ordering::SeqCst);
            session.unsafe_mode.store(unsafe_enabled, Ordering::SeqCst);
            true
        } else {
            false
        }
    };

    if !has_active_session {
        log::info!(
            "Updated execution mode for persisted session '{}' without active runtime state",
            session_id
        );
    }

    if let Some(include_hard_approvals) = mode.include_hard_approvals() {
        if has_active_session {
            super::approvals::approve_all_pending_tool_approvals(
                manager,
                session_id,
                include_hard_approvals,
            )
            .await?;
        }
    }

    crate::agent::tauri_events::emit_resource_updated(
        "session",
        "update",
        Some(session_id.to_string()),
    );

    Ok(())
}
