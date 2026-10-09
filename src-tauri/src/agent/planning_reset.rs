//! Narrow planning resets that must not wipe the whole session plan.

use crate::agent::tauri_events::emit_resource_updated;
use crate::repositories::PlanningRepository;
use crate::state::try_get_planning_repository;

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
