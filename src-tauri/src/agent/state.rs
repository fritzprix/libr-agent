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

    /// Context registry for read-only information providers
    pub context_registry: Arc<ContextRegistry>,
}
