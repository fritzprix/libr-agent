use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingIssueAction {
    Ignore,
    CancelAndRetry { next_retry_count: u32 },
    CancelAndFail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingIssueOutcome {
    Ignored,
    Retried {
        retry_count: u32,
    },
    /// Budget exhausted for a recoverable bad response; caller should continue
    /// with the original completion (e.g. guided tool-error close).
    FallThrough,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonProductiveCompletionAction {
    Retry { next_retry_count: u32 },
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonProductiveCompletionReason {
    RepeatedThinkingLoop {
        response_message_id: String,
        observed_tail_chars: usize,
        pattern_length: usize,
        repetition_count: usize,
    },
    RepeatedTextLoop {
        response_message_id: String,
        observed_tail_chars: usize,
        pattern_length: usize,
        repetition_count: usize,
    },
    ThinkingOnlyCompletion {
        assistant_message_id: String,
    },
    MalformedToolCallArguments {
        assistant_message_id: String,
        tool_names: Vec<String>,
        /// Primary failure kind from the first invalid tool call (for logging).
        parse_kind: String,
    },
    ReasoningBudgetExceeded {
        response_message_id: String,
        observed_tail_chars: usize,
        /// floor(max_tokens * 0.9) from the frontend report.
        reasoning_budget_tokens: usize,
        estimated_thinking_tokens: usize,
    },
    MalformedToolMarkup {
        assistant_message_id: String,
        tool_name: String,
    },
}

/// Payload for a malformed/truncated tool-call-arguments recovery attempt.
pub struct MalformedToolArgsIncident {
    pub assistant_message_id: String,
    pub tool_names: Vec<String>,
    pub parse_kind: String,
}

pub(super) struct RecoveryContext<'a> {
    pub(super) session_repo: &'a Arc<dyn SessionRepository>,
    pub(super) active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub(super) proxy_manager: &'a Arc<MCPServiceProxyManager>,
    pub(super) app_handle: &'a AppHandle,
}
