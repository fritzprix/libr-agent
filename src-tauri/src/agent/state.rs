use crate::agent::context::registry::ContextRegistry;
use crate::commands::messages_commands::Message;
use crate::repositories::SessionMetadata;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Maximum number of messages to keep in memory cache (sliding window)
pub const MAX_CACHED_MESSAGES: usize = 1000;

/// Tracks the state of pending tool executions for a conversational turn
#[derive(Debug)]
pub struct PendingToolExecution {
    pub total_expected: usize,
    pub results: Vec<Message>,
    /// Maps tool_call_id to tool_name for event emission
    pub tool_names: HashMap<String, String>,
}

/// Pending events waiting to be processed by the workflow
#[derive(Debug, Clone)]
pub enum PendingEvent {
    Message(String), // Stores Message ID
                     // Future extensions: ToolApproval, etc.
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

    /// Drain all pending messages and return their IDs
    pub fn drain_messages(&mut self) -> Vec<String> {
        // Currently only Message variant exists.
        // We drain all events and extract IDs.
        // If future variants are added, this logic must be updated to filter/preserve them.
        self.events
            .drain(..)
            .map(|event| {
                let PendingEvent::Message(id) = event;
                id
            })
            .collect()
    }

    pub fn count(&self) -> usize {
        self.events.len()
    }

    #[allow(dead_code)]
    pub fn has_pending(&self) -> bool {
        !self.events.is_empty()
    }
}

/// Represents an active agent session with its runtime state
#[derive(Debug)]
pub struct AgentSession {
    pub metadata: SessionMetadata,
    pub is_running: bool,
    /// Cancellation token to abort running workflows
    pub cancellation_token: CancellationToken,
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

    /// Context registry for read-only information providers
    pub context_registry: Arc<ContextRegistry>,
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
