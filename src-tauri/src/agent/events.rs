use crate::agent::llm::types::{AgentRuntimeError, CompactStateEvent};
use crate::agent::runtime_state::SessionRuntimeState;
use crate::models::chat::Message;
use crate::repositories::SessionStatus;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreflightTokenMetrics {
    pub conservative_prompt_tokens: usize,
    pub prompt_anchored_total_tokens: usize,
    pub safe_input_token_limit: usize,
    pub measured_output_tokens_reserve: usize,
    pub effective_input_budget: usize,
    pub total_budget_tokens: usize,
    pub system_prompt_tokens: usize,
    pub tools_tokens: usize,
    pub selected_message_count: usize,
    pub compact_summary_injected: bool,
    pub preserved_calibration_ratio: Option<f64>,
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
        approval_kind: crate::agent::state::PendingApprovalKind,
        request_id: Option<String>,
        description: Option<String>,
        input_preview: Option<String>,
    },

    /// Remote-channel-friendly permission relay request emitted for external bridges
    #[serde(rename_all = "camelCase")]
    ChannelPermissionRequest {
        session_id: String,
        request_id: String,
        tool_call_id: String,
        tool_name: String,
        approval_kind: crate::agent::state::PendingApprovalKind,
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

    /// Structured runtime-state update for session initialization/readiness.
    #[serde(rename_all = "camelCase")]
    SessionRuntimeStateUpdated {
        session_id: String,
        runtime_state: SessionRuntimeState,
    },

    /// Backend-owned preflight token estimate used to decide compaction.
    #[serde(rename_all = "camelCase")]
    PreflightTokenMetricsUpdated {
        session_id: String,
        metrics: PreflightTokenMetrics,
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

pub(crate) fn summarize_agent_event(event: &AgentEvent) -> String {
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
        AgentEvent::SessionRuntimeStateUpdated {
            session_id,
            runtime_state,
        } => format!(
            "SessionRuntimeStateUpdated(session={session_id}, sequence={}, phase={:?}, proxy_mode={:?})",
            runtime_state.sequence, runtime_state.phase, runtime_state.proxy.mode
        ),
        AgentEvent::PreflightTokenMetricsUpdated {
            session_id,
            metrics,
        } => format!(
            "PreflightTokenMetricsUpdated(session={session_id}, conservative_prompt_tokens={}, prompt_anchored_total_tokens={}, measured_output_tokens_reserve={}, total_budget_tokens={}, effective_input_budget={}, safe_input_token_limit={})",
            metrics.conservative_prompt_tokens,
            metrics.prompt_anchored_total_tokens,
            metrics.measured_output_tokens_reserve,
            metrics.total_budget_tokens,
            metrics.effective_input_budget,
            metrics.safe_input_token_limit
        ),
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
