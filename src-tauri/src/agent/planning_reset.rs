//! Narrow planning resets that must not wipe the whole session (`/clear`).

use crate::agent::tauri_events::emit_resource_updated;
use crate::repositories::PlanningRepository;
use crate::state::{try_get_active_sessions, try_get_planning_repository};

/// Clear goal + todos + scratchpad, invalidate the stable prompt cache, emit UI clear.
///
/// Narrower than `/clear` / [`crate::agent::session_manager::reset::reset_session`]:
/// messages, compact context, and browser sessions are left alone.
///
/// Returns `Err` when the planning repository is unset or the DB clear fails.
/// Callers that must not fail the user-visible tool (e.g. `reportResult`) should
/// use [`clear_planning_state_after_report_result_success`] instead.
pub async fn clear_planning_session_state(session_id: &str) -> Result<(), String> {
    let planning_repo = try_get_planning_repository().ok_or_else(|| {
        format!(
            "planning repository not initialized (session {})",
            session_id
        )
    })?;

    planning_repo
        .clear_session(session_id)
        .await
        .map_err(|e| format!("Failed to clear planning state: {e}"))?;

    // Invalidate cached system prompt so the next Think sees empty planning context.
    if let Some(sessions) = try_get_active_sessions() {
        let sessions = sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            *session.cached_stable_prompt.write().await = None;
        }
    }

    emit_resource_updated("planning", "clear", Some(session_id.to_string()));
    Ok(())
}

/// Best-effort full planning clear after settle-eligible `ui__reportResult(success)`.
///
/// Logs and returns when the repository is unset or clear fails — never fails the
/// MCP result. Skipped for `partial` / `blocked` and for missing-export rejects.
pub async fn clear_planning_state_after_report_result_success(session_id: &str) {
    if let Err(error) = clear_planning_session_state(session_id).await {
        log::warn!(
            "Failed to clear planning state after reportResult(success) for session {}: {}",
            session_id,
            error
        );
    }
}

/// Clear session todos after a successful compaction.
///
/// Keeps the active goal and scratchpad so the compact handoff owns residue/Done
/// while the durable objective remains. Planning service context is volatile
/// (`Medium`), so the next Think refetches an empty todo list from the DB.
///
/// Best-effort: skips when the planning repository is unset; logs repository
/// errors without failing compaction. Emits `action=clear` so the UI refresh
/// path in AgentChatContext matches scheduled planning clears.
pub async fn clear_todos_after_successful_compaction(session_id: &str) {
    let Some(planning_repo) = try_get_planning_repository() else {
        log::warn!(
            "Skipping planning todo clear after compaction for session {}: planning repository not initialized",
            session_id
        );
        return;
    };
    if let Err(error) = planning_repo.clear_todos(session_id).await {
        log::warn!(
            "Failed to clear planning todos after compaction for session {}: {}",
            session_id,
            error
        );
        return;
    }

    emit_resource_updated("planning", "clear", Some(session_id.to_string()));
}
