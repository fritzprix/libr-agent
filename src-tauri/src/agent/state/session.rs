use super::{
    CompactionRuntimeState, PendingApprovalData, PendingEventManager, PendingToolExecution,
};
use crate::agent::concurrency::ActiveAgentPermit;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::llm::types::CompactionParentRequest;
use crate::agent::poll_tracker::PollTracker;
use crate::models::chat::Message;
use crate::repositories::{CompactContextRecord, SessionMetadata};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatusTransition {
    ToStatus(crate::repositories::SessionStatus),
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

    /// Counts Rust-owned recovery retries after client-side reasoning-budget
    /// aborts (thinking reached ~90% of max output tokens). Independent from
    /// repeated-thinking / text-loop counters. No prompt nudge is injected.
    pub reasoning_budget_retry_count: Arc<RwLock<u32>>,

    /// Counts recovery retries after a final turn that was only text-formatted
    /// tool-call markup (`<function=NAME>…</function>`) with no structured call.
    /// Reset to 0 when a turn completes normally.
    pub malformed_tool_markup_retry_count: Arc<RwLock<u32>>,

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

    /// Previous-turn volatile session-context snapshot for Phase 2 lagged inject
    /// (`SC(state @ tn−1)` before `response(tn−1)`). Ephemeral — not persisted to DB.
    pub last_session_context_snapshot: Arc<RwLock<Option<String>>>,

    /// Completions since the last force-fresh SC inject (heartbeat).
    pub session_context_turns_since_force_fresh: Arc<RwLock<u32>>,

    /// Session-scoped resample attempt counter keyed by loop fingerprint
    /// (single-tool signature or batch fingerprint). Independent of DB history
    /// length so resample budget survives discarded assistant turns.
    pub tool_loop_resample_attempts: Arc<RwLock<HashMap<String, usize>>>,

    /// In-band poll snapshot trackers keyed by `{tool}:{resource_id}`.
    pub tool_poll_trackers: Arc<RwLock<HashMap<String, PollTracker>>>,

    /// Optional session-scoped override for context-management `maxInputContext`.
    /// When `Some(n)` with `n > 0`, compaction / orchestration use `n` instead of
    /// the global settings value. Runtime-only (not persisted).
    pub max_input_context_override: Option<usize>,
}

impl AgentSession {
    /// In-memory SSOT for approval policy. Persisted copy lives in the session repo.
    #[inline]
    pub fn execution_mode(&self) -> crate::execution_mode::ExecutionMode {
        self.metadata.execution_mode
    }

    /// Construct a new idle in-memory session shell for create / resume / recover.
    ///
    /// Field layout stays flat — do not wrap groups in `Arc` (prior Arc-grouping
    /// attempt was reverted).
    pub fn new(
        metadata: SessionMetadata,
        context_registry: Arc<ContextRegistry>,
        compact_context: Option<CompactContextRecord>,
    ) -> Self {
        Self {
            metadata,
            is_running: false,
            active_permit: None,
            status_transition: Arc::new(RwLock::new(None)),
            transition_lock: Arc::new(Mutex::new(())),
            cancellation_token: CancellationToken::new(),
            cancel_pending: Arc::new(AtomicBool::new(false)),
            pending_execution: None,
            messages: Arc::new(RwLock::new(Vec::new())),
            cache_initialized: Arc::new(AtomicBool::new(false)),
            last_synced_at: Arc::new(RwLock::new(None)),
            repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
            repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
            bad_tool_args_retry_count: Arc::new(RwLock::new(0)),
            bad_tool_args_incident_count: Arc::new(RwLock::new(0)),
            reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
            malformed_tool_markup_retry_count: Arc::new(RwLock::new(0)),
            pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            context_registry,
            compact_context: Arc::new(RwLock::new(compact_context)),
            compaction: CompactionRuntimeState::new(),
            expected_response_id: Arc::new(RwLock::new(None)),
            cached_stable_prompt: Arc::new(RwLock::new(None)),
            last_completion_request: Arc::new(RwLock::new(None)),
            last_submitted_input_message_id: Arc::new(RwLock::new(None)),
            last_session_context_snapshot: Arc::new(RwLock::new(None)),
            session_context_turns_since_force_fresh: Arc::new(RwLock::new(0)),
            tool_loop_resample_attempts: Arc::new(RwLock::new(HashMap::new())),
            tool_poll_trackers: Arc::new(RwLock::new(HashMap::new())),
            max_input_context_override: None,
        }
    }

    /// Clears the in-memory message cache, invalidates the cached system prompt,
    /// and resets transient tool approval/execution states.
    pub async fn clear(&mut self) {
        self.messages.write().await.clear();
        *self.cached_stable_prompt.write().await = None;
        self.pending_approvals.write().await.clear();
        self.pending_execution = None;
        *self.compact_context.write().await = None;
        self.compaction.clear_runtime_state(true).await;
        self.compaction.clear_telemetry_events().await;
        self.pending_events.write().await.clear();
        *self.expected_response_id.write().await = None;
        *self.last_completion_request.write().await = None;
        *self.last_submitted_input_message_id.write().await = None;
        *self.last_session_context_snapshot.write().await = None;
        *self.session_context_turns_since_force_fresh.write().await = 0;
        self.tool_loop_resample_attempts.write().await.clear();
        self.tool_poll_trackers.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::{CompactionKind, CompactionPhase, PendingApprovalKind, PendingEvent};
    use std::collections::HashSet;

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
                status_reason: None,
            },
            is_running: true,
            active_permit: None,
            status_transition: Arc::new(RwLock::new(None)),
            transition_lock: Arc::new(tokio::sync::Mutex::new(())),
            cancellation_token: CancellationToken::new(),
            cancel_pending: Arc::new(AtomicBool::new(false)),
            pending_execution: None,
            messages: Arc::new(RwLock::new(Vec::new())),
            cache_initialized: Arc::new(AtomicBool::new(true)),
            last_synced_at: Arc::new(RwLock::new(None)),
            repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
            repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
            bad_tool_args_retry_count: Arc::new(RwLock::new(0)),
            bad_tool_args_incident_count: Arc::new(RwLock::new(0)),
            reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
            malformed_tool_markup_retry_count: Arc::new(RwLock::new(0)),
            pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            context_registry: Arc::new(ContextRegistry::new()),
            compact_context: Arc::new(RwLock::new(None)),
            compaction: CompactionRuntimeState::new(),
            expected_response_id: Arc::new(RwLock::new(None)),
            cached_stable_prompt: Arc::new(RwLock::new(None)),
            last_completion_request: Arc::new(RwLock::new(None)),
            last_submitted_input_message_id: Arc::new(RwLock::new(None)),
            last_session_context_snapshot: Arc::new(RwLock::new(None)),
            session_context_turns_since_force_fresh: Arc::new(RwLock::new(0)),
            tool_loop_resample_attempts: Arc::new(RwLock::new(HashMap::new())),
            tool_poll_trackers: Arc::new(RwLock::new(HashMap::new())),
            max_input_context_override: None,
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
            deferred_history_append: Vec::new(),
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
        session
            .compaction
            .try_begin(CompactionKind::Manual, Some("tail-1".to_string()), 12345)
            .await;
        assert!(session.compaction.snapshot().await.is_in_flight());

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
