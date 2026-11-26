//! User-facing settlement error text and event-emission helper.

use crate::agent::events::{AgentEvent, AgentEventDispatcher};

pub(super) fn user_facing_persist_session_missing_error() -> String {
    "The session could not be found while saving the final response.".to_string()
}

pub(super) fn user_facing_persist_storage_error() -> String {
    "The final response could not be saved. The session has been marked as failed.".to_string()
}

pub(super) fn user_facing_workflow_event_emit_error(event_label: &str) -> String {
    format!(
        "The session state was updated, but the UI could not be notified ({event_label}). \
         Refresh or check the session status manually."
    )
}

pub(super) fn emit_settlement_agent_event(
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    event: AgentEvent,
    event_label: &str,
) -> Result<(), String> {
    dispatcher.emit_agent_event(event).map_err(|error| {
        log::error!(
            "Failed to emit {event_label} for session {session_id}: {error}. \
                 Session status was already updated; StatusChanged should have reached the UI."
        );
        user_facing_workflow_event_emit_error(event_label)
    })
}
