use crate::agent::concurrency::ActiveAgentPermit;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::llm::types::CompactionParentRequest;
use crate::agent::types::ToolCall;
use crate::models::chat::Message;
use crate::repositories::{CompactContextRecord, SessionMetadata};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::oneshot;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

/// Maximum number of messages to keep in memory cache (sliding window)
pub const MAX_CACHED_MESSAGES: usize = 1000;

/// Tracks the state of pending tool executions for a conversational turn
#[derive(Debug)]
pub struct PendingToolExecution {
    pub message_id: String,
    pub total_expected: usize,
    pub results: Vec<Message>,
    /// Maps tool_call_id to tool_name for event emission
    pub tool_names: HashMap<String, String>,
    /// Tool call IDs expected for the current message execution
    pub expected_tool_call_ids: HashSet<String>,
    /// Tool call IDs already completed for the current message execution
    pub completed_tool_call_ids: HashSet<String>,
}

/// Pending events waiting to be processed by the workflow
#[derive(Debug, Clone)]
pub enum PendingEvent {
    Message(String), // Stores Message ID
}

/// Manages pending events for a session
#[derive(Debug, Default)]
pub struct PendingEventManager {
    events: Vec<PendingEvent>,
}

impl PendingEventManager {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn add(&mut self, event: PendingEvent) {
        self.events.push(event);
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Drain all pending messages and return their IDs
    pub fn drain_messages(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        self.events.retain(|event| match event {
            PendingEvent::Message(id) => {
                messages.push(id.clone());
                false
            }
        });
        messages
    }

    pub fn count(&self) -> usize {
        self.events.len()
    }

    #[allow(dead_code)]
    pub fn has_pending(&self) -> bool {
        !self.events.is_empty()
    }
}

/// Data for a pending tool execution approval
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingApprovalKind {
    Standard,
    Hard,
}

/// Data for a pending tool execution approval
#[derive(Debug)]
pub struct PendingApprovalData {
    pub sender: oneshot::Sender<bool>,
    pub tool_name: String,
    pub arguments: String,
    pub approval_kind: PendingApprovalKind,
    pub request_id: Option<String>,
    pub description: Option<String>,
    pub input_preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatusTransition {
    ToStatus(crate::repositories::SessionStatus),
}

#[derive(Debug, Clone)]
pub enum DeferredWorkflowStep {
    RequestCompletion,
    ExecuteToolCalls {
        assistant_message_id: String,
        tool_calls: Vec<ToolCall>,
    },
    FinalizeWorkflow {
        reason: crate::agent::events::WorkflowCompletionReason,
    },
}

#[derive(Debug, Clone)]
pub enum CompactionKind {
    Manual,
    Preflight,
    PostResponse { deferred_step: DeferredWorkflowStep },
}

impl CompactionKind {
    pub fn mode_label(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Preflight => "preflight",
            Self::PostResponse { .. } => "post-response",
        }
    }

    pub fn blocks_workflow(&self) -> bool {
        !matches!(self, Self::Manual)
    }

    pub fn resumes_completion_after_compact(&self) -> bool {
        matches!(self, Self::Preflight)
    }
}

#[derive(Debug, Clone)]
pub struct InFlightCompaction {
    pub kind: CompactionKind,
    pub current_tail_id: Option<String>,
    pub started_at_ms: i64,
}

#[derive(Debug, Clone)]
pub enum CompactionPhase {
    Idle,
    InFlight(InFlightCompaction),
}

#[derive(Debug, Clone)]
pub struct CompactionSnapshot {
    pub phase: CompactionPhase,
    pub last_compacted_tail_id: Option<String>,
    pub retry_attempt: u32,
    pub recovery_phase: CompactionRecoveryPhase,
}

impl CompactionSnapshot {
    pub fn started_at_ms(&self) -> Option<i64> {
        match &self.phase {
            CompactionPhase::Idle => None,
            CompactionPhase::InFlight(in_flight) => Some(in_flight.started_at_ms),
        }
    }

    pub fn mode_label(&self) -> &'static str {
        match &self.phase {
            CompactionPhase::Idle => "manual",
            CompactionPhase::InFlight(in_flight) => in_flight.kind.mode_label(),
        }
    }

    pub fn blocks_workflow(&self) -> bool {
        match &self.phase {
            CompactionPhase::Idle => false,
            CompactionPhase::InFlight(in_flight) => in_flight.kind.blocks_workflow(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CompactionBeginOutcome {
    Started,
    AlreadyInFlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionReuseOutcome {
    NotInFlight,
    NoChange,
    Promoted,
}

#[derive(Debug, Clone)]
pub enum CompactionResumeAction {
    Nothing,
    ResumeCompletion,
    RunDeferred(DeferredWorkflowStep),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionRecoveryPhase {
    CacheAligned,
    OverflowRecovery,
    DegradedTools,
}

#[derive(Debug, Clone)]
pub struct CompactionRuntimeState {
    /// Closed runtime phase model for active compaction work.
    phase: Arc<RwLock<CompactionPhase>>,

    /// The ID of the last message in the stack at the moment compaction was triggered.
    /// This survives successful settlement so future triggers can skip same-tail work.
    last_compacted_tail_id: Arc<RwLock<Option<String>>>,

    /// Retry ladder state for budget-related compaction overflows.
    retry_attempt: Arc<RwLock<u32>>,

    /// Phase ladder for compaction overflow recovery.
    recovery_phase: Arc<RwLock<CompactionRecoveryPhase>>,
}

impl CompactionRuntimeState {
    pub fn new() -> Self {
        Self {
            phase: Arc::new(RwLock::new(CompactionPhase::Idle)),
            last_compacted_tail_id: Arc::new(RwLock::new(None)),
            retry_attempt: Arc::new(RwLock::new(0)),
            recovery_phase: Arc::new(RwLock::new(CompactionRecoveryPhase::CacheAligned)),
        }
    }

    pub fn with_test_state(phase: CompactionPhase, last_tail_id: Option<String>) -> Self {
        Self {
            phase: Arc::new(RwLock::new(phase)),
            last_compacted_tail_id: Arc::new(RwLock::new(last_tail_id)),
            retry_attempt: Arc::new(RwLock::new(0)),
            recovery_phase: Arc::new(RwLock::new(CompactionRecoveryPhase::CacheAligned)),
        }
    }

    pub async fn snapshot(&self) -> CompactionSnapshot {
        CompactionSnapshot {
            phase: self.phase.read().await.clone(),
            last_compacted_tail_id: self.last_compacted_tail_id.read().await.clone(),
            retry_attempt: *self.retry_attempt.read().await,
            recovery_phase: *self.recovery_phase.read().await,
        }
    }

    pub async fn last_compacted_tail_id(&self) -> Option<String> {
        self.last_compacted_tail_id.read().await.clone()
    }

    pub async fn clear_runtime_state(&self, clear_last_compacted_tail_id: bool) {
        *self.phase.write().await = CompactionPhase::Idle;

        if clear_last_compacted_tail_id {
            *self.last_compacted_tail_id.write().await = None;
        }
    }

    pub async fn retry_attempt(&self) -> u32 {
        *self.retry_attempt.read().await
    }

    pub async fn recovery_phase(&self) -> CompactionRecoveryPhase {
        *self.recovery_phase.read().await
    }

    pub async fn increment_retry_attempt(&self) -> u32 {
        let mut retry_attempt = self.retry_attempt.write().await;
        *retry_attempt += 1;
        *retry_attempt
    }

    pub async fn reset_retry_attempt(&self) {
        *self.retry_attempt.write().await = 0;
    }

    pub async fn transition_to_overflow_recovery(&self) {
        *self.retry_attempt.write().await = 0;
        *self.recovery_phase.write().await = CompactionRecoveryPhase::OverflowRecovery;
    }

    pub async fn transition_to_degraded_tools(&self) {
        *self.retry_attempt.write().await = 0;
        *self.recovery_phase.write().await = CompactionRecoveryPhase::DegradedTools;
    }

    pub async fn reset_recovery_progress(&self) {
        *self.retry_attempt.write().await = 0;
        *self.recovery_phase.write().await = CompactionRecoveryPhase::CacheAligned;
    }

    pub async fn set_recovery_progress(
        &self,
        recovery_phase: CompactionRecoveryPhase,
        retry_attempt: u32,
    ) {
        *self.retry_attempt.write().await = retry_attempt;
        *self.recovery_phase.write().await = recovery_phase;
    }

    pub async fn try_begin(
        &self,
        kind: CompactionKind,
        current_tail_id: Option<String>,
        started_at_ms: i64,
    ) -> CompactionBeginOutcome {
        let mut phase = self.phase.write().await;
        if matches!(&*phase, CompactionPhase::InFlight(_)) {
            return CompactionBeginOutcome::AlreadyInFlight;
        }

        *phase = CompactionPhase::InFlight(InFlightCompaction {
            kind,
            current_tail_id: current_tail_id.clone(),
            started_at_ms,
        });
        *self.last_compacted_tail_id.write().await = current_tail_id;
        CompactionBeginOutcome::Started
    }

    pub async fn arm_resume_completion(&self) -> CompactionReuseOutcome {
        let mut phase = self.phase.write().await;
        match &mut *phase {
            CompactionPhase::Idle => CompactionReuseOutcome::NotInFlight,
            CompactionPhase::InFlight(in_flight) => match &in_flight.kind {
                CompactionKind::Manual => {
                    in_flight.kind = CompactionKind::Preflight;
                    CompactionReuseOutcome::Promoted
                }
                CompactionKind::Preflight | CompactionKind::PostResponse { .. } => {
                    CompactionReuseOutcome::NoChange
                }
            },
        }
    }

    pub async fn attach_deferred_workflow_step(
        &self,
        deferred_step: DeferredWorkflowStep,
    ) -> CompactionReuseOutcome {
        let mut phase = self.phase.write().await;
        match &mut *phase {
            CompactionPhase::Idle => CompactionReuseOutcome::NotInFlight,
            CompactionPhase::InFlight(in_flight) => match &in_flight.kind {
                CompactionKind::PostResponse { .. } => CompactionReuseOutcome::NoChange,
                CompactionKind::Manual | CompactionKind::Preflight => {
                    in_flight.kind = CompactionKind::PostResponse { deferred_step };
                    CompactionReuseOutcome::Promoted
                }
            },
        }
    }

    pub async fn complete_success(&self) -> CompactionResumeAction {
        self.reset_recovery_progress().await;
        match std::mem::replace(&mut *self.phase.write().await, CompactionPhase::Idle) {
            CompactionPhase::Idle => CompactionResumeAction::Nothing,
            CompactionPhase::InFlight(in_flight) => match in_flight.kind {
                CompactionKind::Manual => CompactionResumeAction::Nothing,
                CompactionKind::Preflight => CompactionResumeAction::ResumeCompletion,
                CompactionKind::PostResponse { deferred_step } => {
                    CompactionResumeAction::RunDeferred(deferred_step)
                }
            },
        }
    }

    pub fn is_settled(&self) -> bool {
        match self.phase.try_read() {
            Ok(phase) => matches!(&*phase, CompactionPhase::Idle),
            Err(_) => false,
        }
    }
}

impl Default for CompactionRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an active agent session with its runtime state
#[derive(Debug)]
pub struct AgentSession {
    pub metadata: SessionMetadata,
    pub is_running: bool,
    pub active_permit: Option<ActiveAgentPermit>,
    pub status_transition: Arc<RwLock<Option<SessionStatusTransition>>>,
    pub transition_lock: Arc<Mutex<()>>,
    /// Cancellation token to abort running workflows
    pub cancellation_token: CancellationToken,

    /// YOLO mode: execute tools without requiring approval
    pub yolo_mode: Arc<AtomicBool>,

    /// Unsafe mode: bypass approval and policy enforcement
    pub unsafe_mode: Arc<AtomicBool>,

    /// Cancel-pending flag to block post-cancel recursion/re-entry
    pub cancel_pending: Arc<AtomicBool>,

    /// State of current turn's tool execution
    pub pending_execution: Option<PendingToolExecution>,

    /// In-memory message cache (loaded once on init, updated in-place)
    /// Thread-safe: Arc allows shared ownership, RwLock allows concurrent reads
    pub messages: Arc<RwLock<Vec<Message>>>,

    /// Flag indicating if cache has been initialized from DB
    pub cache_initialized: Arc<AtomicBool>,

    /// Last DB sync timestamp (for debugging/monitoring)
    pub last_synced_at: Arc<RwLock<Option<SystemTime>>>,

    /// Counts Rust-owned recovery retries after non-productive completions
    /// (streaming repeated-thinking loops or completed thinking-only turns)
    /// during a single workflow turn. Reset when a new workflow starts or the
    /// assistant produces meaningful progress.
    pub repeated_thinking_retry_count: Arc<RwLock<u32>>,

    /// Pending events (messages, approvals, etc.) waiting for workflow processing
    pub pending_events: Arc<RwLock<PendingEventManager>>,

    /// Channels for pending tool execution approvals
    /// Maps tool_call_id to PendingApprovalData (which unblocks the workflow with a bool)
    pub pending_approvals: Arc<RwLock<HashMap<String, PendingApprovalData>>>,

    /// Context registry for read-only information providers
    pub context_registry: Arc<ContextRegistry>,

    /// Compact context for the session (SP17)
    pub compact_context: Arc<RwLock<Option<CompactContextRecord>>>,

    /// Transient runtime-only compaction orchestration state.
    pub compaction: CompactionRuntimeState,

    /// The ID generated for the next assistant message response.
    /// Shared with the frontend via CompletionRequest so streaming and persisted messages match.
    pub expected_response_id: Arc<RwLock<Option<String>>>,

    /// Cached stable system prompt prefix (sections 1–4: agent identity, persona,
    /// workspace instructions, session context). These sections are immutable within
    /// a session so we build them once and reuse on every LLM call to avoid redundant
    /// JSON parsing and filesystem I/O.
    pub cached_stable_prompt: Arc<RwLock<Option<String>>>,

    /// Exact prompt-layout fields from the latest emitted completion request.
    /// Reused by compaction so the summarization call preserves provider cache prefixes.
    pub last_completion_request: Arc<RwLock<Option<CompactionParentRequest>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_event_manager_flow() {
        let mut manager = PendingEventManager::new();
        assert!(!manager.has_pending());

        manager.add(PendingEvent::Message("msg1".into()));
        manager.add(PendingEvent::Message("msg2".into()));
        assert!(manager.has_pending());
        assert_eq!(manager.count(), 2);

        let messages = manager.drain_messages();
        assert_eq!(messages, vec!["msg1", "msg2"]);

        assert!(!manager.has_pending());
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_clear_events() {
        let mut manager = PendingEventManager::new();
        manager.add(PendingEvent::Message("msg1".into()));

        manager.clear();
        assert!(!manager.has_pending());
        assert_eq!(manager.count(), 0);
        assert!(manager.drain_messages().is_empty());
    }
}
