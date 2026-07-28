use crate::agent::concurrency::ActiveAgentPermit;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::llm::types::{CompactRequest, CompactionParentRequest};
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

    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Drain the next pending message id (FIFO), leaving the rest queued.
    pub fn drain_one_message(&mut self) -> Option<String> {
        let index = self
            .events
            .iter()
            .position(|event| matches!(event, PendingEvent::Message(_)))?;
        match self.events.remove(index) {
            PendingEvent::Message(id) => Some(id),
        }
    }

    /// Re-insert a message at the FIFO front after a failed durable claim/cancel.
    pub fn restore_front_message(&mut self, message_id: String) {
        self.events.insert(0, PendingEvent::Message(message_id));
    }

    /// Re-insert multiple messages at the FIFO front, preserving order.
    pub fn restore_front_pending_messages(&mut self, message_ids: &[String]) {
        for message_id in message_ids.iter().rev() {
            self.events
                .insert(0, PendingEvent::Message(message_id.clone()));
        }
    }

    /// Whether a specific message id is currently waiting.
    pub fn contains_message(&self, message_id: &str) -> bool {
        self.events.iter().any(|event| match event {
            PendingEvent::Message(id) => id == message_id,
        })
    }

    /// Remove a specific pending message id. Returns true when found.
    pub fn remove_message(&mut self, message_id: &str) -> bool {
        let before = self.events.len();
        self.events.retain(|event| match event {
            PendingEvent::Message(id) => id != message_id,
        });
        self.events.len() != before
    }

    /// Snapshot of pending message ids in FIFO order.
    pub fn message_ids(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|event| match event {
                PendingEvent::Message(id) => id.clone(),
            })
            .collect()
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
pub enum CompactionKind {
    Manual,
    Preflight,
}

impl CompactionKind {
    pub fn mode_label(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Preflight => "preflight",
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
    pub summary_retry_count: u32,
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

    /// Retry counter for low-quality or empty compaction summaries.
    summary_retry_count: Arc<RwLock<u32>>,

    /// Most recent compact request payload, retained while compaction is in flight so
    /// the backend can re-emit it without involving the frontend error path.
    current_request: Arc<RwLock<Option<CompactRequest>>>,
}

impl CompactionRuntimeState {
    pub fn new() -> Self {
        Self {
            phase: Arc::new(RwLock::new(CompactionPhase::Idle)),
            last_compacted_tail_id: Arc::new(RwLock::new(None)),
            retry_attempt: Arc::new(RwLock::new(0)),
            recovery_phase: Arc::new(RwLock::new(CompactionRecoveryPhase::CacheAligned)),
            summary_retry_count: Arc::new(RwLock::new(0)),
            current_request: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_test_state(phase: CompactionPhase, last_tail_id: Option<String>) -> Self {
        Self {
            phase: Arc::new(RwLock::new(phase)),
            last_compacted_tail_id: Arc::new(RwLock::new(last_tail_id)),
            retry_attempt: Arc::new(RwLock::new(0)),
            recovery_phase: Arc::new(RwLock::new(CompactionRecoveryPhase::CacheAligned)),
            summary_retry_count: Arc::new(RwLock::new(0)),
            current_request: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn snapshot(&self) -> CompactionSnapshot {
        CompactionSnapshot {
            phase: self.phase.read().await.clone(),
            last_compacted_tail_id: self.last_compacted_tail_id.read().await.clone(),
            retry_attempt: *self.retry_attempt.read().await,
            recovery_phase: *self.recovery_phase.read().await,
            summary_retry_count: *self.summary_retry_count.read().await,
        }
    }

    pub async fn last_compacted_tail_id(&self) -> Option<String> {
        self.last_compacted_tail_id.read().await.clone()
    }

    pub async fn clear_in_flight_state(&self, clear_last_compacted_tail_id: bool) {
        *self.phase.write().await = CompactionPhase::Idle;
        *self.summary_retry_count.write().await = 0;
        *self.current_request.write().await = None;

        if clear_last_compacted_tail_id {
            *self.last_compacted_tail_id.write().await = None;
        }
    }

    pub async fn clear_runtime_state(&self, clear_last_compacted_tail_id: bool) {
        self.clear_in_flight_state(clear_last_compacted_tail_id)
            .await;
        self.reset_recovery_progress().await;
    }

    pub async fn retry_attempt(&self) -> u32 {
        *self.retry_attempt.read().await
    }

    pub async fn recovery_phase(&self) -> CompactionRecoveryPhase {
        *self.recovery_phase.read().await
    }

    pub async fn summary_retry_count(&self) -> u32 {
        *self.summary_retry_count.read().await
    }

    pub async fn increment_summary_retry_count(&self) -> u32 {
        let mut retry_count = self.summary_retry_count.write().await;
        *retry_count += 1;
        *retry_count
    }

    pub async fn reset_summary_retry_count(&self) {
        *self.summary_retry_count.write().await = 0;
    }

    pub async fn current_request(&self) -> Option<CompactRequest> {
        self.current_request.read().await.clone()
    }

    pub async fn set_current_request(&self, request: CompactRequest) {
        *self.current_request.write().await = Some(request);
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
        *self.summary_retry_count.write().await = 0;
        *self.current_request.write().await = None;
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
                CompactionKind::Preflight => CompactionReuseOutcome::NoChange,
            },
        }
    }

    pub async fn complete_success(&self) -> CompactionResumeAction {
        self.reset_recovery_progress().await;
        self.reset_summary_retry_count().await;
        *self.current_request.write().await = None;
        match std::mem::replace(&mut *self.phase.write().await, CompactionPhase::Idle) {
            CompactionPhase::Idle => CompactionResumeAction::Nothing,
            CompactionPhase::InFlight(in_flight) => match in_flight.kind {
                CompactionKind::Manual => CompactionResumeAction::Nothing,
                CompactionKind::Preflight => CompactionResumeAction::ResumeCompletion,
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

    /// Counts Rust-owned recovery retries after non-productive thinking
    /// completions (streaming repeated-thinking loops or thinking-only turns)
    /// during a single workflow turn. Reset when a new workflow starts or the
    /// assistant produces meaningful progress.
    pub repeated_thinking_retry_count: Arc<RwLock<u32>>,

    /// Counts Rust-owned recovery retries after streaming repeated-text loops.
    /// Independent from `repeated_thinking_retry_count`.
    pub repeated_text_loop_retry_count: Arc<RwLock<u32>>,

    /// Counts Rust-owned recovery retries after malformed/truncated tool-call
    /// argument JSON in a completion. Independent from thinking/text counters.
    /// Not reset on FallThrough — only on workflow start or a clean valid batch.
    pub bad_tool_args_retry_count: Arc<RwLock<u32>>,

    /// Counts FallThrough incidents for malformed tool args in the current
    /// workflow. Hard-stops after a fixed cap to bound unbounded truncated loops.
    pub bad_tool_args_incident_count: Arc<RwLock<u32>>,

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

    /// Last raw input message ID included in the most recent emitted completion request.
    /// Used to persist provider-reported prompt tokens onto the correct checkpoint message.
    pub last_submitted_input_message_id: Arc<RwLock<Option<String>>>,
}

impl AgentSession {
    /// Clears the in-memory message cache, invalidates the cached system prompt,
    /// and resets transient tool approval/execution states.
    pub async fn clear(&mut self) {
        self.messages.write().await.clear();
        *self.cached_stable_prompt.write().await = None;
        self.pending_approvals.write().await.clear();
        self.pending_execution = None;
        *self.compact_context.write().await = None;
        self.compaction.clear_runtime_state(true).await;
        self.pending_events.write().await.clear();
        *self.expected_response_id.write().await = None;
        *self.last_completion_request.write().await = None;
        *self.last_submitted_input_message_id.write().await = None;
    }
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
        assert_eq!(manager.message_ids(), vec!["msg1", "msg2"]);

        let first = manager.drain_one_message();
        assert_eq!(first.as_deref(), Some("msg1"));
        assert_eq!(manager.count(), 1);

        assert!(manager.remove_message("msg2"));
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

    fn build_test_session(session_id: &str) -> AgentSession {
        use crate::repositories::SessionMetadata;
        use crate::repositories::SessionStatus;
        use std::sync::atomic::AtomicBool;
        use tokio_util::sync::CancellationToken;

        let now = chrono::Utc::now().timestamp_millis();
        AgentSession {
            metadata: SessionMetadata {
                id: session_id.to_string(),
                name: None,
                status: SessionStatus::Busy,
                model: "gpt-5.4".to_string(),
                provider: "openai".to_string(),
                assistant_id: None,
                parent_session_id: None,
                lineage_id: None,
                depth: None,
                max_depth: None,
                max_fanout: None,
                org_id: None,
                org_name: None,
                org_root_session_id: None,
                created_at: now,
                updated_at: now,
                last_viewed_at: None,
                last_message_at: None,
                last_attention_at: None,
                last_attention_reason: None,
                is_bookmarked: false,
                execution_mode: crate::execution_mode::ExecutionMode::Normal,
                workspace_override: None,
                workspace_isolation:
                    crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
                docker_config: None,
                docker_container_name: None,
                docker_host_workspace_path: None,
            },
            is_running: true,
            active_permit: None,
            status_transition: Arc::new(RwLock::new(None)),
            transition_lock: Arc::new(tokio::sync::Mutex::new(())),
            cancellation_token: CancellationToken::new(),
            yolo_mode: Arc::new(AtomicBool::new(false)),
            unsafe_mode: Arc::new(AtomicBool::new(false)),
            cancel_pending: Arc::new(AtomicBool::new(false)),
            pending_execution: None,
            messages: Arc::new(RwLock::new(Vec::new())),
            cache_initialized: Arc::new(AtomicBool::new(true)),
            last_synced_at: Arc::new(RwLock::new(None)),
            repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
            repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
            bad_tool_args_retry_count: Arc::new(RwLock::new(0)),
            bad_tool_args_incident_count: Arc::new(RwLock::new(0)),
            pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            context_registry: Arc::new(ContextRegistry::new()),
            compact_context: Arc::new(RwLock::new(None)),
            compaction: CompactionRuntimeState::new(),
            expected_response_id: Arc::new(RwLock::new(None)),
            cached_stable_prompt: Arc::new(RwLock::new(None)),
            last_completion_request: Arc::new(RwLock::new(None)),
            last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        }
    }

    #[tokio::test]
    async fn test_agent_session_clear() {
        let mut session = build_test_session("test-sess");

        // 1. Populate some data
        session
            .messages
            .write()
            .await
            .push(Message::new_user_message(
                "test-sess".to_string(),
                "hello".to_string(),
                None,
                None,
            ));
        *session.cached_stable_prompt.write().await = Some("cached-prompt".to_string());
        *session.expected_response_id.write().await = Some("expected-id".to_string());
        *session.last_submitted_input_message_id.write().await = Some("input-id".to_string());
        *session.last_completion_request.write().await = Some(CompactionParentRequest {
            model: "test-model".to_string(),
            provider: "test-provider".to_string(),
            system_prompt: Some("system".to_string()),
            session_context: None,
            available_tools: None,
        });
        session.pending_approvals.write().await.insert(
            "call-1".to_string(),
            PendingApprovalData {
                sender: tokio::sync::oneshot::channel().0,
                tool_name: "test_tool".to_string(),
                arguments: "{}".to_string(),
                approval_kind: PendingApprovalKind::Standard,
                request_id: None,
                description: None,
                input_preview: None,
            },
        );
        session.pending_execution = Some(PendingToolExecution {
            message_id: "exec-1".to_string(),
            total_expected: 0,
            tool_names: HashMap::new(),
            expected_tool_call_ids: HashSet::new(),
            completed_tool_call_ids: HashSet::new(),
        });
        *session.compact_context.write().await = Some(CompactContextRecord {
            id: "cc-1".to_string(),
            session_id: "test-sess".to_string(),
            to_id: "msg-1".to_string(),
            condensed_count: Some(5),
            summary: "summary".to_string(),
            created_at: 0,
        });
        session
            .pending_events
            .write()
            .await
            .add(PendingEvent::Message("event-1".to_string()));

        // Run compaction active work (change phase to something non-Idle)
        *session.compaction.phase.write().await = CompactionPhase::InFlight(InFlightCompaction {
            kind: CompactionKind::Manual,
            current_tail_id: Some("tail-1".to_string()),
            started_at_ms: 12345,
        });

        // 2. Run clear
        session.clear().await;

        // 3. Assert cleared
        assert!(session.messages.read().await.is_empty());
        assert!(session.cached_stable_prompt.read().await.is_none());
        assert!(session.expected_response_id.read().await.is_none());
        assert!(session
            .last_submitted_input_message_id
            .read()
            .await
            .is_none());
        assert!(session.last_completion_request.read().await.is_none());
        assert!(session.pending_approvals.read().await.is_empty());
        assert!(session.pending_execution.is_none());
        assert!(session.compact_context.read().await.is_none());
        assert!(!session.pending_events.read().await.has_pending());

        let snapshot = session.compaction.snapshot().await;
        assert!(matches!(snapshot.phase, CompactionPhase::Idle));
    }
}
