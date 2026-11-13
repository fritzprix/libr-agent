use crate::agent::concurrency::ActiveAgentPermit;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::context::time_location::TimeLocationContextProvider;
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub mod approvals;
mod channel;
mod compact;
pub mod execution_mode;
mod lifecycle;
mod llm;
mod message_injection;
mod reload;
mod reset;
mod tools;
pub mod user_prompt;
mod workflow;

pub use approvals::PendingApprovalSummary;
pub use user_prompt::pending_user_prompt_in;

pub use channel::format_channel_payload_for_test;
pub use compact::build_compaction_hard_fallback_summary_for_testing;
pub use compact::clamp_compact_summary_to_context_limit;
pub use compact::clear_message_prompt_token_checkpoint_for_testing;
pub use compact::compaction_fallback_artifact_relative_path_for_testing;
pub use compact::dump_partial_compaction_output_if_any;
pub use compact::dump_raw_compaction_output_if_needed;
pub use compact::handle_compact_error_with_dispatcher;
pub use compact::should_retry_budget_related_blocking_compaction;
pub use compact::validate_compact_summary_for_testing;
pub use compact::CompactContextView;
pub use compact::CompactStreamMeta;
pub use compact::CompactSummaryClampResult;

use crate::agent::repositories::AgentRepositories;
use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::pending_queue_repository::PendingQueueRepository;

/// Manages agent sessions and their workflows.
///
/// Public facade for session runtime. Domain logic lives in focused modules:
/// - `crate::agent::lifecycle` — create, resume, recover, delete, status
/// - `crate::agent::workflow` / `llm` / `tools` — execution pipeline
/// - `session_manager::{approvals,channel,compact,execution_mode,lifecycle,llm,message_injection,reload,reset,tools,user_prompt,workflow}`
///
/// Workspace path resolution stays in `crate::session` (e.g. `resolve_session_workspace_dir`).
#[derive(Clone)]
pub struct AgentSessionManager {
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: AppHandle,
    proxy_manager: Arc<MCPServiceProxyManager>,
    session_repo: Arc<dyn SessionRepository>,
    repositories: AgentRepositories,
    context_registry: Arc<ContextRegistry>,
}

// Manual Debug implementation since dyn Trait doesn't auto-implement Debug
impl std::fmt::Debug for AgentSessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentSessionManager")
            .field("active_sessions", &"<Arc<RwLock<HashMap>>>")
            .field("app_handle", &"<AppHandle>")
            .field("proxy_manager", &"<Arc<MCPServiceProxyManager>>")
            .field("session_repo", &"<Arc<dyn SessionRepository>>")
            .field("repositories", &self.repositories)
            .field("context_registry", &"<Arc<ContextRegistry>>")
            .finish()
    }
}

impl AgentSessionManager {
    /// Create a new AgentSessionManager with dependency injection
    pub fn new(
        app_handle: AppHandle,
        proxy_manager: Arc<MCPServiceProxyManager>,
        session_repo: Arc<dyn SessionRepository>,
    ) -> Self {
        Self::new_with_repositories(
            app_handle,
            proxy_manager,
            AgentRepositories::new(session_repo),
        )
    }

    /// Create a new AgentSessionManager with an explicit AgentRepositories bundle
    pub fn new_with_repositories(
        app_handle: AppHandle,
        proxy_manager: Arc<MCPServiceProxyManager>,
        repositories: AgentRepositories,
    ) -> Self {
        // Initialize context registry with providers
        let mut registry = ContextRegistry::new();

        // Register time/location context provider (high priority)
        registry.register(Box::new(TimeLocationContextProvider::new()));

        log::info!(
            "✅ Context registry initialized with {} providers",
            registry.provider_count()
        );

        let session_repo = repositories.session.clone();

        Self {
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
            proxy_manager,
            session_repo,
            repositories,
            context_registry: Arc::new(registry),
        }
    }

    /// Access the injected repository bundle.
    pub fn repositories(&self) -> &AgentRepositories {
        &self.repositories
    }

    /// Access the active sessions map for read/write handlers.
    pub(crate) fn active_sessions(&self) -> &Arc<RwLock<HashMap<String, AgentSession>>> {
        &self.active_sessions
    }

    /// Access the Tauri app handle.
    pub(crate) fn app_handle(&self) -> &AppHandle {
        &self.app_handle
    }

    /// Resolve message repository: returns injected repository or falls back to global state.
    pub fn message_repo(&self) -> Option<Arc<dyn MessageRepositoryTrait>> {
        self.repositories.message_repo()
    }

    /// Resolve pending queue repository: returns injected repository or falls back to global state.
    pub fn pending_queue_repo(&self) -> Option<Arc<dyn PendingQueueRepository>> {
        self.repositories.pending_queue_repo()
    }

    /// Resolve compact context repository: returns injected repository or falls back to global state.
    pub fn compact_context_repo(&self) -> Option<Arc<dyn CompactContextRepository>> {
        self.repositories.compact_context_repo()
    }

    /// Return an owned clone of the `Arc` wrapping the active sessions map.
    /// Used at startup to register a shared reference in the global `state` module so that builtin MCP tools can look up per-session cancellation tokens.
    pub fn active_sessions_arc(&self) -> Arc<RwLock<HashMap<String, AgentSession>>> {
        self.active_sessions.clone()
    }

    pub async fn take_active_session_permit(&self, session_id: &str) -> Option<ActiveAgentPermit> {
        let mut active = self.active_sessions.write().await;
        active
            .get_mut(session_id)
            .and_then(|session| session.active_permit.take())
    }

    pub async fn restore_active_session_permit(
        &self,
        session_id: &str,
        permit: ActiveAgentPermit,
    ) -> Result<(), String> {
        let mut active = self.active_sessions.write().await;
        let session = active
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        session.active_permit = Some(permit);
        Ok(())
    }

    /// Clone self for use in async tasks
    pub fn clone_for_task(&self) -> Self {
        Self {
            active_sessions: self.active_sessions.clone(),
            app_handle: self.app_handle.clone(),
            proxy_manager: self.proxy_manager.clone(),
            session_repo: self.session_repo.clone(),
            repositories: self.repositories.clone(),
            context_registry: self.context_registry.clone(),
        }
    }
}
