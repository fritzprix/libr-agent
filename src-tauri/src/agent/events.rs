use crate::commands::messages_commands::Message;
use crate::repositories::SessionStatus;
use log::info;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Events emitted from Rust Agent runtime to TypeScript Frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    /// Workflow started for a session
    #[serde(rename_all = "camelCase")]
    WorkflowStarted { session_id: String },

    /// Workflow completed successfully
    #[serde(rename_all = "camelCase")]
    WorkflowCompleted { session_id: String },

    /// Workflow encountered an error
    #[serde(rename_all = "camelCase")]
    WorkflowError { session_id: String, error: String },

    /// Session status changed
    #[serde(rename_all = "camelCase")]
    StatusChanged {
        session_id: String,
        status: SessionStatus,
    },

    /// Message added to session (includes full message for immediate UI update)
    #[serde(rename_all = "camelCase")]
    MessageAdded {
        session_id: String,
        message: Box<Message>,
    },

    /// Tool execution started
    #[serde(rename_all = "camelCase")]
    ToolExecutionStarted {
        session_id: String,
        tool_name: String,
    },

    /// Tool execution completed
    #[serde(rename_all = "camelCase")]
    ToolExecutionCompleted {
        session_id: String,
        tool_name: String,
        success: bool,
    },

    /// Session initialization step update
    #[serde(rename_all = "camelCase")]
    InitializationStep {
        session_id: String,
        step: String,
        status: String,
    },
}

/// Emit an agent event to the frontend
pub fn emit_agent_event(app_handle: &AppHandle, event: AgentEvent) -> Result<(), String> {
    // Use emit_to() to broadcast to all windows (Tauri 2.x requirement)
    // EventTarget::app() sends to all webviews
    info!("Emitting agent event: {:#?}", event);
    app_handle
        .emit_to(tauri::EventTarget::app(), "agent:event", event)
        .map_err(|e| format!("Failed to emit agent event: {}", e))
}
