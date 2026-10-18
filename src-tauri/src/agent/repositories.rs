use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::pending_queue_repository::PendingQueueRepository;
use crate::repositories::session_repository::SessionRepository;
use std::sync::Arc;

/// Bundle of repository dependencies used across agent session workflows and lifecycle operations.
///
/// Designed to enable incremental dependency injection (DI) and test seam isolation without
/// rewriting the entire application architecture or breaking process-wide defaults.
#[derive(Clone)]
pub struct AgentRepositories {
    pub session: Arc<dyn SessionRepository>,
    pub message: Option<Arc<dyn MessageRepository>>,
    pub pending_queue: Option<Arc<dyn PendingQueueRepository>>,
    pub compact_context: Option<Arc<dyn CompactContextRepository>>,
}

// Manual Debug implementation since trait objects don't auto-implement Debug
impl std::fmt::Debug for AgentRepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRepositories")
            .field("session", &"<Arc<dyn SessionRepository>>")
            .field(
                "message",
                &self
                    .message
                    .as_ref()
                    .map(|_| "<Arc<dyn MessageRepository>>"),
            )
            .field(
                "pending_queue",
                &self
                    .pending_queue
                    .as_ref()
                    .map(|_| "<Arc<dyn PendingQueueRepository>>"),
            )
            .field(
                "compact_context",
                &self
                    .compact_context
                    .as_ref()
                    .map(|_| "<Arc<dyn CompactContextRepository>>"),
            )
            .finish()
    }
}

impl AgentRepositories {
    /// Create a new `AgentRepositories` with a required session repository.
    pub fn new(session: Arc<dyn SessionRepository>) -> Self {
        Self {
            session,
            message: None,
            pending_queue: None,
            compact_context: None,
        }
    }

    /// Builder method to attach an explicit message repository.
    pub fn with_message(mut self, repo: Arc<dyn MessageRepository>) -> Self {
        self.message = Some(repo);
        self
    }

    /// Builder method to attach an explicit pending queue repository.
    pub fn with_pending_queue(mut self, repo: Arc<dyn PendingQueueRepository>) -> Self {
        self.pending_queue = Some(repo);
        self
    }

    /// Builder method to attach an explicit compact context repository.
    pub fn with_compact_context(mut self, repo: Arc<dyn CompactContextRepository>) -> Self {
        self.compact_context = Some(repo);
        self
    }

    /// Resolve message repository: returns injected repository or falls back to global state.
    pub fn message_repo(&self) -> Option<Arc<dyn MessageRepository>> {
        self.message.clone().or_else(|| {
            crate::state::try_get_message_repository()
                .map(|r| Arc::new((*r).clone()) as Arc<dyn MessageRepository>)
        })
    }

    /// Resolve pending queue repository: returns injected repository or falls back to global state.
    pub fn pending_queue_repo(&self) -> Option<Arc<dyn PendingQueueRepository>> {
        self.pending_queue.clone().or_else(|| {
            crate::state::try_get_pending_queue_repository()
                .map(|r| Arc::new((*r).clone()) as Arc<dyn PendingQueueRepository>)
        })
    }

    /// Resolve compact context repository: returns injected repository or falls back to global state.
    pub fn compact_context_repo(&self) -> Option<Arc<dyn CompactContextRepository>> {
        self.compact_context.clone().or_else(|| {
            crate::state::try_get_compact_context_repository()
                .map(|r| Arc::new((*r).clone()) as Arc<dyn CompactContextRepository>)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::in_memory_session_repository::InMemorySessionRepository;

    #[test]
    fn test_agent_repositories_builder_and_resolution() {
        let session_repo = Arc::new(InMemorySessionRepository::new());
        let repos = AgentRepositories::new(session_repo.clone());

        assert!(repos.message.is_none());
        assert!(repos.pending_queue.is_none());
        assert!(repos.compact_context.is_none());

        let debug_str = format!("{:?}", repos);
        assert!(debug_str.contains("AgentRepositories"));
    }
}
