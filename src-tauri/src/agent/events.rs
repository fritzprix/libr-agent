use crate::agent::llm::types::{
    AgentRuntimeError, CompactStateEvent, CompactStatePhase, CompletionCancelRequest,
};
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
    /// Soft cancel: leave the session paused so the user can resume.
    Cancelled,
    /// Hard stop / terminate: session returns to idle (no resume stack).
    Terminated,
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
        /// Stored reason for the new status; `None` when the status has no reason.
        status_reason: Option<String>,
    },

    /// Message added to session (includes full message for immediate UI update)
    #[serde(rename_all = "camelCase")]
    MessageAdded {
        session_id: String,
        message: Box<Message>,
    },

    /// Messages removed from session transcript (e.g. regenerate last turn)
    #[serde(rename_all = "camelCase")]
    MessagesRemoved {
        session_id: String,
        message_ids: Vec<String>,
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

    /// A `secret` or `question` prompt is waiting for the local user (#2543).
    ///
    /// The payload never carries a secret value: only the variable name and the
    /// label. The answer returns through the backend channel, not this event.
    #[serde(rename_all = "camelCase")]
    UserPromptRequested {
        session_id: String,
        request_id: String,
        /// Tagged with `kind`, so the dialog switches on `payload.kind` alone.
        payload: crate::agent::state::UserPromptPayload,
    },

    /// A user prompt was answered, cancelled, timed out, or dropped by Stop.
    #[serde(rename_all = "camelCase")]
    UserPromptResolved {
        session_id: String,
        request_id: String,
        /// `answered` | `cancelled` | `timeout` | `restart`
        outcome: String,
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

    /// Loop-prevention circuit breaker fired (soft recovery, escalate, or hard break).
    #[serde(rename_all = "camelCase")]
    CircuitBreakerTriggered {
        session_id: String,
        tool_name: String,
        count: usize,
        /// `hardBreak` | `softRecovery` | `errorEscalate` | `repeatedBatch` | `duplicateInBatch`
        action: String,
    },

    /// Waiting user prompts that are not yet part of the active turn.
    #[serde(rename_all = "camelCase")]
    PendingQueueUpdated {
        session_id: String,
        messages: Vec<Message>,
    },
}

pub trait AgentEventDispatcher: Send + Sync {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String>;
    fn emit_compact_state(&self, event: CompactStateEvent) -> Result<(), String>;
    fn emit_completion_cancel(&self, request: CompletionCancelRequest) -> Result<(), String>;

    /// Compact started for one session. Built on [`Self::emit_compact_state`] so an
    /// implementation names its transport once and the phase shape stays in one place
    /// (#2772: the compact-event family reaches the runtime through this trait only).
    fn emit_compact_started(
        &self,
        session_id: String,
        session_name: Option<String>,
        awaiting_compact: bool,
    ) -> Result<(), String> {
        self.emit_compact_state(CompactStateEvent {
            session_id,
            session_name,
            compacting: true,
            awaiting_compact,
            phase: CompactStatePhase::Started,
            error: None,
        })
    }

    /// Compact left the started phase. Same reasoning as
    /// [`Self::emit_compact_started`].
    fn emit_compact_finished(
        &self,
        session_id: String,
        session_name: Option<String>,
        phase: CompactStatePhase,
        error: Option<String>,
    ) -> Result<(), String> {
        self.emit_compact_state(CompactStateEvent {
            session_id,
            session_name,
            compacting: false,
            awaiting_compact: false,
            phase,
            error,
        })
    }
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
        AgentEvent::StatusChanged {
            session_id, status, ..
        } => {
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
        AgentEvent::MessagesRemoved {
            session_id,
            message_ids,
        } => format!(
            "MessagesRemoved(session={}, count={})",
            session_id,
            message_ids.len()
        ),
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
        AgentEvent::UserPromptRequested {
            session_id,
            request_id,
            payload,
        } => format!(
            "UserPromptRequested(session={session_id}, request_id={request_id}, {})",
            payload.describe()
        ),
        AgentEvent::UserPromptResolved {
            session_id,
            request_id,
            outcome,
        } => format!(
            "UserPromptResolved(session={session_id}, request_id={request_id}, outcome={outcome})"
        ),
        AgentEvent::ResourceUpdated {
            resource_type,
            action,
            resource_id,
        } => format!(
            "ResourceUpdated(type={resource_type}, action={action}, resource_id={})",
            resource_id.as_deref().unwrap_or("-")
        ),
        AgentEvent::CircuitBreakerTriggered {
            session_id,
            tool_name,
            count,
            action,
        } => format!(
            "CircuitBreakerTriggered(session={session_id}, tool={tool_name}, count={count}, action={action})"
        ),
        AgentEvent::PendingQueueUpdated {
            session_id,
            messages,
        } => format!(
            "PendingQueueUpdated(session={session_id}, count={})",
            messages.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Records what the trait's default methods build, so the compact phase shape
    /// is pinned without a Tauri handle (#2772).
    #[derive(Default)]
    struct RecordingDispatcher {
        states: Mutex<Vec<CompactStateEvent>>,
    }

    impl AgentEventDispatcher for RecordingDispatcher {
        fn emit_agent_event(&self, _event: AgentEvent) -> Result<(), String> {
            Ok(())
        }

        fn emit_compact_state(&self, event: CompactStateEvent) -> Result<(), String> {
            self.states.lock().unwrap().push(event);
            Ok(())
        }

        fn emit_completion_cancel(&self, _request: CompletionCancelRequest) -> Result<(), String> {
            Ok(())
        }
    }

    fn recorded(dispatcher: &RecordingDispatcher) -> Vec<CompactStateEvent> {
        dispatcher.states.lock().unwrap().clone()
    }

    #[test]
    fn compact_started_default_builds_started_state() {
        let dispatcher = RecordingDispatcher::default();
        dispatcher
            .emit_compact_started("s-1".to_string(), Some("Chat".to_string()), true)
            .unwrap();

        let states = recorded(&dispatcher);
        assert_eq!(states.len(), 1);
        let state = &states[0];
        assert_eq!(state.session_id, "s-1");
        assert_eq!(state.session_name.as_deref(), Some("Chat"));
        assert!(state.compacting, "started must keep the compacting flag on");
        assert!(state.awaiting_compact);
        assert!(matches!(state.phase, CompactStatePhase::Started));
        assert_eq!(state.error, None);
    }

    #[test]
    fn compact_finished_default_clears_compacting_and_keeps_phase_and_error() {
        let dispatcher = RecordingDispatcher::default();
        dispatcher
            .emit_compact_finished(
                "s-2".to_string(),
                None,
                CompactStatePhase::Failed,
                Some("boom".to_string()),
            )
            .unwrap();

        let states = recorded(&dispatcher);
        assert_eq!(states.len(), 1);
        let state = &states[0];
        assert_eq!(state.session_id, "s-2");
        assert!(!state.compacting, "finished must clear the compacting flag");
        assert!(!state.awaiting_compact);
        assert!(matches!(state.phase, CompactStatePhase::Failed));
        assert_eq!(state.error.as_deref(), Some("boom"));
    }
}
