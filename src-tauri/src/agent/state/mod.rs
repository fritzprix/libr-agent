//! In-memory runtime state for agent sessions.

mod compaction;
mod message_cache;
mod pending;
mod session;
mod user_prompt;

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
pub use user_prompt::{
    admit_user_prompt, await_user_prompt, await_user_prompt_with_timeout,
    new_user_prompt_slot, pending_user_prompt_summary, secret_outcome_message,
    take_any_pending_user_prompt, take_pending_user_prompt, wait_for_user_prompt,
    PendingUserPrompt, PendingUserPromptSummary, UserPromptAnswer, UserPromptKind,
    UserPromptOutcome, UserPromptPayload, UserPromptSlot, USER_PROMPT_ALREADY_PENDING_MESSAGE,
    USER_PROMPT_CANCELLED_MESSAGE, USER_PROMPT_RESTART_MESSAGE, USER_PROMPT_TIMEOUT_MESSAGE,
    USER_PROMPT_TIMEOUT_SECS,
};
