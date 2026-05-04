use crate::agent::events::{summarize_agent_event, AgentEvent, AgentEventDispatcher};
use crate::agent::llm::types::{
    CompactRequest, CompactStateEvent, CompactStatePhase, CompletionCancelRequest,
};
use log::info;
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
pub struct TauriEventDispatcher {
    app_handle: AppHandle,
}

impl TauriEventDispatcher {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl AgentEventDispatcher for TauriEventDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        emit_agent_event(&self.app_handle, event)
    }

    fn emit_compact_state(&self, event: CompactStateEvent) -> Result<(), String> {
        emit_compact_state(&self.app_handle, event)
    }
}

/// Emit an agent event to the frontend
pub fn emit_agent_event(app_handle: &AppHandle, event: AgentEvent) -> Result<(), String> {
    // Use emit_to() to broadcast to all windows (Tauri 2.x requirement)
    // EventTarget::app() sends to all webviews
    info!("Emitting agent event: {}", summarize_agent_event(&event));
    app_handle
        .emit_to(tauri::EventTarget::app(), "agent:event", event)
        .map_err(|e| format!("Failed to emit agent event: {}", e))
}

pub fn emit_compact_state(app_handle: &AppHandle, event: CompactStateEvent) -> Result<(), String> {
    app_handle
        .emit("llm:compact-state", event)
        .map_err(|e| format!("Failed to emit llm:compact-state: {}", e))
}

pub fn emit_compact_started(
    app_handle: &AppHandle,
    session_id: impl Into<String>,
    session_name: Option<String>,
    awaiting_compact: bool,
) -> Result<(), String> {
    emit_compact_state(
        app_handle,
        CompactStateEvent {
            session_id: session_id.into(),
            session_name,
            compacting: true,
            awaiting_compact,
            phase: CompactStatePhase::Started,
            error: None,
        },
    )
}

pub fn emit_compact_finished(
    app_handle: &AppHandle,
    session_id: impl Into<String>,
    session_name: Option<String>,
    phase: CompactStatePhase,
    error: Option<String>,
) -> Result<(), String> {
    emit_compact_state(
        app_handle,
        CompactStateEvent {
            session_id: session_id.into(),
            session_name,
            compacting: false,
            awaiting_compact: false,
            phase,
            error,
        },
    )
}

pub fn emit_compact_request(app_handle: &AppHandle, event: CompactRequest) -> Result<(), String> {
    app_handle
        .emit("llm:compact-request", event)
        .map_err(|e| format!("Failed to emit llm:compact-request: {}", e))
}

pub fn emit_completion_cancel(
    app_handle: &AppHandle,
    event: CompletionCancelRequest,
) -> Result<(), String> {
    app_handle
        .emit("llm:completion-cancel", event)
        .map_err(|e| format!("Failed to emit llm:completion-cancel: {}", e))
}

/// Emit a resource update event (convenience wrapper)
///
/// This is a shorthand for emitting ResourceUpdated events from builtin tools.
/// Falls back silently if AppHandle is not available (e.g., during tests).
pub fn emit_resource_updated(resource_type: &str, action: &str, resource_id: Option<String>) {
    if let Some(app_handle) = crate::state::get_app_handle() {
        let event = AgentEvent::ResourceUpdated {
            resource_type: resource_type.to_string(),
            action: action.to_string(),
            resource_id,
        };

        if let Err(error) = emit_agent_event(app_handle, event) {
            log::warn!("Failed to emit resource update event: {}", error);
        }
    } else {
        log::debug!(
            "AppHandle not available, skipping resource update event (resource_type: {}, action: {})",
            resource_type,
            action
        );
    }
}
