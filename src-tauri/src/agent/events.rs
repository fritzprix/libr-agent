use crate::repositories::SessionStatus;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Events emitted from Rust Agent runtime to TypeScript Frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    /// Workflow started for a session
    WorkflowStarted { session_id: String },

    /// Workflow completed successfully
    WorkflowCompleted { session_id: String },

    /// Workflow encountered an error
    WorkflowError { session_id: String, error: String },

    /// Session status changed
    StatusChanged {
        session_id: String,
        status: SessionStatus,
    },

    /// Message added to session
    MessageAdded {
        session_id: String,
        message_id: String,
    },

    /// Tool execution started
    ToolExecutionStarted {
        session_id: String,
        tool_name: String,
    },

    /// Tool execution completed
    ToolExecutionCompleted {
        session_id: String,
        tool_name: String,
        success: bool,
    },
}

/// Emit an agent event to the frontend
pub fn emit_agent_event(app_handle: &AppHandle, event: AgentEvent) -> Result<(), String> {
    app_handle
        .emit("agent:event", event)
        .map_err(|e| format!("Failed to emit agent event: {}", e))
}
