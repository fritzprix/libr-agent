use super::AgentSessionManager;
use crate::execution_mode::ExecutionMode;

/// Compare-and-restore helper for a failed persist after an optimistic in-memory write.
///
/// If another writer already moved `current` past `written`, keep `current`.
pub fn revert_mode_if_unchanged(
    current: ExecutionMode,
    written: ExecutionMode,
    previous: ExecutionMode,
) -> ExecutionMode {
    if current == written {
        previous
    } else {
        current
    }
}

/// Persist and apply session execution mode.
///
/// # SSOT
/// Active sessions: `AgentSession.metadata.execution_mode` is the only in-memory
/// authority. The DB mirrors it for cold open / HTTP GET.
///
/// # Race notes
/// - In-memory update runs under the active-sessions write lock so readers never
///   observe a torn mode.
/// - Memory is updated **before** DB so tool approval / warm UI see the new mode
///   immediately.
/// - On DB failure we revert memory only if it still equals the mode we wrote
///   (compare-and-restore), so a concurrent successful `set_execution_mode` is
///   not clobbered by a late failure rollback.
pub async fn set_execution_mode(
    manager: &AgentSessionManager,
    session_id: &str,
    mode: ExecutionMode,
) -> Result<(), String> {
    let previous_mode = {
        let mut active = manager.active_sessions.write().await;
        if let Some(session) = active.get_mut(session_id) {
            let previous = session.metadata.execution_mode;
            session.metadata.execution_mode = mode;
            Some(previous)
        } else {
            None
        }
    };

    if let Err(error) = manager
        .session_repo
        .update_execution_mode(session_id, mode)
        .await
    {
        if let Some(previous) = previous_mode {
            let mut active = manager.active_sessions.write().await;
            if let Some(session) = active.get_mut(session_id) {
                session.metadata.execution_mode =
                    revert_mode_if_unchanged(session.metadata.execution_mode, mode, previous);
            }
        }
        return Err(format!(
            "Failed to update session execution mode: {}",
            error
        ));
    }

    if previous_mode.is_none() {
        log::info!(
            "Updated execution mode for persisted session '{}' to {} (no active runtime)",
            session_id,
            mode.as_str()
        );
    } else {
        log::info!(
            "Set execution mode for session '{}' to {}",
            session_id,
            mode.as_str()
        );
    }

    if let Some(include_hard_approvals) = mode.include_hard_approvals() {
        if previous_mode.is_some() {
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
