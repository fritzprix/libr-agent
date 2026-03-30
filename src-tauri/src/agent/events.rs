use crate::agent::llm::types::{AgentRuntimeError, CompactStateEvent};
use crate::models::chat::Message;
use crate::repositories::SessionStatus;
use log::info;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Initialization step status
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InitializationStatus {
    Running,
    Complete,
    Error,
}

/// Reason why a workflow completed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowCompletionReason {
    Natural,
    RecurringStop,
    Cancelled,
}

/// Events emitted from Rust Agent runtime to TypeScript Frontend
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    /// Workflow started for a session
    #[serde(rename_all = "camelCase")]
    WorkflowStarted { session_id: String },

    /// Workflow completed successfully
    #[serde(rename_all = "camelCase")]
    WorkflowCompleted {
        session_id: String,
        reason: WorkflowCompletionReason,
    },

    /// Workflow encountered an error
    #[serde(rename_all = "camelCase")]
    WorkflowError {
        session_id: String,
        error: AgentRuntimeError,
    },

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

    /// Tool execution blocked waiting for user approval
    #[serde(rename_all = "camelCase")]
    ToolExecutionRequiresApproval {
        session_id: String,
        tool_call_id: String,
        tool_name: String,
        arguments: String,
    },

    /// Remote-channel-friendly permission relay request emitted for external bridges
    #[serde(rename_all = "camelCase")]
    ChannelPermissionRequest {
        session_id: String,
        request_id: String,
        tool_call_id: String,
        tool_name: String,
        description: String,
        input_preview: String,
    },

    /// Pending approval resolved by either the local UI or a remote channel bridge
    #[serde(rename_all = "camelCase")]
    ToolExecutionApprovalResolved {
        session_id: String,
        tool_call_id: String,
        approved: bool,
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
        status: InitializationStatus,
    },

    /// Resource updated (assistants, MCP servers, playbooks, etc.)
    /// Emitted when builtin tools modify global resources
    #[serde(rename_all = "camelCase")]
    ResourceUpdated {
        /// Type of resource: "assistant" | "mcpServer" | "playbook"
        resource_type: String,
        /// Action performed: "create" | "update" | "delete"
        action: String,
        /// Optional resource identifier
        resource_id: Option<String>,
    },
}

pub trait AgentEventDispatcher: Send + Sync {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String>;
    fn emit_compact_state(&self, event: CompactStateEvent) -> Result<(), String>;
}

#[derive(Clone)]
pub struct TauriEventDispatcher {
    app_handle: AppHandle,
}

fn summarize_agent_event(event: &AgentEvent) -> String {
    match event {
        AgentEvent::WorkflowStarted { session_id } => {
            format!("WorkflowStarted(session={session_id})")
        }
        AgentEvent::WorkflowCompleted { session_id, reason } => {
            format!("WorkflowCompleted(session={session_id}, reason={reason:?})")
        }
        AgentEvent::WorkflowError { session_id, .. } => {
            format!("WorkflowError(session={session_id})")
        }
        AgentEvent::StatusChanged { session_id, status } => {
            format!("StatusChanged(session={session_id}, status={status:?})")
        }
        AgentEvent::MessageAdded { session_id, message } => {
            let media_items = message
                .content
                .iter()
                .filter(|item| matches!(item, crate::mcp::types::MCPContent::Image { .. } | crate::mcp::types::MCPContent::Audio { .. }))
                .count();
            format!(
                "MessageAdded(session={}, message={}, role={}, content_items={}, media_items={})",
                session_id,
                message.id,
                message.role,
                message.content.len(),
                media_items
            )
        }
        AgentEvent::ToolExecutionStarted {
            session_id,
            tool_name,
        } => format!("ToolExecutionStarted(session={session_id}, tool={tool_name})"),
        AgentEvent::ToolExecutionRequiresApproval {
            session_id,
            tool_call_id,
            tool_name,
            ..
        } => format!(
            "ToolExecutionRequiresApproval(session={session_id}, tool_call_id={tool_call_id}, tool={tool_name})"
        ),
        AgentEvent::ChannelPermissionRequest {
            session_id,
            request_id,
            tool_call_id,
            tool_name,
            ..
        } => format!(
            "ChannelPermissionRequest(session={session_id}, request_id={request_id}, tool_call_id={tool_call_id}, tool={tool_name})"
        ),
        AgentEvent::ToolExecutionApprovalResolved {
            session_id,
            tool_call_id,
            approved,
        } => format!(
            "ToolExecutionApprovalResolved(session={session_id}, tool_call_id={tool_call_id}, approved={approved})"
        ),
        AgentEvent::ToolExecutionCompleted {
            session_id,
            tool_name,
            success,
        } => format!(
            "ToolExecutionCompleted(session={session_id}, tool={tool_name}, success={success})"
        ),
        AgentEvent::InitializationStep {
            session_id,
            step,
            status,
        } => format!("InitializationStep(session={session_id}, step={step}, status={status:?})"),
        AgentEvent::ResourceUpdated {
            resource_type,
            action,
            resource_id,
        } => format!(
            "ResourceUpdated(type={resource_type}, action={action}, resource_id={})",
            resource_id.as_deref().unwrap_or("-")
        ),
    }
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

        if let Err(e) = emit_agent_event(app_handle, event) {
            log::warn!("Failed to emit resource update event: {}", e);
        }
    } else {
        log::debug!(
            "AppHandle not available, skipping resource update event (resource_type: {}, action: {})",
            resource_type,
            action
        );
    }
}
