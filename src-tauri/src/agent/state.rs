use crate::agent::context::registry::ContextRegistry;
use crate::models::chat::Message;
use crate::repositories::{CompactContextRecord, SessionMetadata};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::oneshot;
use tokio::sync::RwLock;
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
#[derive(Debug)]
pub struct PendingApprovalData {
    pub sender: oneshot::Sender<bool>,
    pub tool_name: String,
    pub arguments: String,
}

/// Represents an active agent session with its runtime state
#[derive(Debug)]
pub struct AgentSession {
    pub metadata: SessionMetadata,
    pub is_running: bool,
    /// Cancellation token to abort running workflows
    pub cancellation_token: CancellationToken,

    /// YOLO mode: execute tools without requiring approval
    pub yolo_mode: Arc<AtomicBool>,

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

    /// Circuit breaker: consecutive thinking-only response count
    /// Reset to 0 when content or tool_calls are generated
    /// Max allowed: 3 (prevents infinite thinking loops)
    pub thinking_only_count: Arc<RwLock<u32>>,

    /// Pending events (messages, approvals, etc.) waiting for workflow processing
    pub pending_events: Arc<RwLock<PendingEventManager>>,

    /// Channels for pending tool execution approvals
    /// Maps tool_call_id to PendingApprovalData (which unblocks the workflow with a bool)
    pub pending_approvals: Arc<RwLock<HashMap<String, PendingApprovalData>>>,

    /// Context registry for read-only information providers
    pub context_registry: Arc<ContextRegistry>,

    /// Compact context for the session (SP17)
    pub compact_context: Arc<RwLock<Option<CompactContextRecord>>>,

    /// Guard: true while a llm:compact-request is in-flight (frontend hasn't returned yet).
    /// Prevents double-triggering compaction within the same session.
    pub compact_in_flight: Arc<AtomicBool>,

    /// The ID of the last message in the stack at the moment compaction was triggered.
    /// On the next Step B evaluation, if messages.last().id still equals this value,
    /// it means no new messages have been added since the last compaction — skip.
    /// Replaced when a new compaction fires with a different tail.
    pub last_compacted_tail_id: Arc<RwLock<Option<String>>>,

    /// True when the current turn is blocked waiting for compaction to finish
    /// before Rust should retry the LLM request.
    pub awaiting_compact_completion: Arc<AtomicBool>,

    /// The ID generated for the next assistant message response.
    /// Shared with the frontend via CompletionRequest so streaming and persisted messages match.
    pub expected_response_id: Arc<RwLock<Option<String>>>,

    /// Cached stable system prompt prefix (sections 1–4: agent identity, persona,
    /// workspace instructions, session context). These sections are immutable within
    /// a session so we build them once and reuse on every LLM call to avoid redundant
    /// JSON parsing and filesystem I/O.
    pub cached_stable_prompt: Arc<RwLock<Option<String>>>,
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
