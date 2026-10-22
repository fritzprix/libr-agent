//! In-memory runtime state for agent sessions.

mod compaction;
mod message_cache;
mod pending;
mod session;

pub use compaction::{
    CompactionBeginOutcome, CompactionKind, CompactionPhase, CompactionRecoveryPhase,
    CompactionResumeAction, CompactionReuseOutcome, CompactionRuntimeState, CompactionSnapshot,
    InFlightCompaction,
};
pub use message_cache::{
    enforce_cached_message_limit, MAX_CACHED_MESSAGES, MESSAGE_CACHE_SOFT_LIMIT,
};
pub use pending::{
    PendingApprovalData, PendingApprovalKind, PendingEvent, PendingEventManager,
    PendingToolExecution,
};
pub use session::{AgentSession, SessionStatusTransition};
