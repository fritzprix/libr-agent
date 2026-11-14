use crate::agent::events::{summarize_agent_event, AgentEvent, AgentEventDispatcher};
use crate::agent::llm::types::{CompactRequest, CompactStateEvent, CompletionCancelRequest};
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

    /// The same dispatcher from a borrowed handle. Runtime code that only ever
    /// holds `&AppHandle` (the llm paths do) takes the dispatcher from here
    /// instead of spelling out the clone at every emit site (#2772).
    pub fn from_handle(app_handle: &AppHandle) -> Self {
        Self {
            app_handle: (*app_handle).clone(),
        }
    }
}

impl AgentEventDispatcher for TauriEventDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        emit_agent_event(&self.app_handle, event)
    }

    fn emit_compact_state(&self, event: CompactStateEvent) -> Result<(), String> {
        emit_compact_state(&self.app_handle, event)
    }

    fn emit_completion_cancel(&self, request: CompletionCancelRequest) -> Result<(), String> {
        // A call running in the Rust executor has no frontend stream to abort.
        crate::agent::llm::rust_executor::cancel(&request.session_id);
        self.app_handle
            .emit("llm:completion-cancel", request)
            .map_err(|e| format!("Failed to emit llm:completion-cancel: {}", e))
    }
}

/// Emit an agent event to the frontend
pub fn emit_agent_event(app_handle: &AppHandle, event: AgentEvent) -> Result<(), String> {
    // Integration tests pass a `tauri::test::mock_app()` handle with no webview
    // event loop; `emit` / `emit_to` on that handle can hang the async runtime.
    // Production always registers the real handle during startup.
    if crate::state::get_app_handle().is_none() {
        log::debug!(
            "Skipping agent event emit (AppHandle not registered): {}",
            summarize_agent_event(&event)
        );
        return Ok(());
    }

    // Use emit_to() to broadcast to all windows (Tauri 2.x requirement)
    // EventTarget::app() sends to all webviews
    info!("Emitting agent event: {}", summarize_agent_event(&event));
    app_handle
        .emit_to(tauri::EventTarget::app(), "agent:event", event)
        .map_err(|e| format!("Failed to emit agent event: {}", e))
}

fn emit_compact_state(app_handle: &AppHandle, event: CompactStateEvent) -> Result<(), String> {
    app_handle
        .emit("llm:compact-state", event)
        .map_err(|e| format!("Failed to emit llm:compact-state: {}", e))
}

pub fn emit_compact_request(app_handle: &AppHandle, event: CompactRequest) -> Result<(), String> {
    if crate::agent::llm::rust_executor::try_dispatch_compaction(app_handle, &event)
        .map_err(|e| e.display_message)?
    {
        return Ok(());
    }
    app_handle
        .emit("llm:compact-request", event)
        .map_err(|e| format!("Failed to emit llm:compact-request: {}", e))
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
