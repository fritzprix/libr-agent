use crate::agent::channel_routing::ChannelRouteCandidate;
use crate::agent::concurrency::ActiveAgentPermit;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::context::time_location::TimeLocationContextProvider;
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::execution_mode::ExecutionMode;
use crate::mcp::types::ChannelNotification;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::{
    CompactContextRecord, SessionListCursor, SessionListPage, SessionMetadata, SessionRepository,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::RwLock;

mod approvals;
mod channel;
mod compact;
pub mod execution_mode;
mod message_injection;
mod reload;
mod reset;

pub use channel::format_channel_payload_for_test;
pub use compact::build_compaction_hard_fallback_summary_for_testing;
pub use compact::clamp_compact_summary_to_context_limit;
pub use compact::clear_message_prompt_token_checkpoint_for_testing;
pub use compact::compaction_fallback_artifact_relative_path_for_testing;
pub use compact::handle_compact_error_with_dispatcher;
pub use compact::should_retry_budget_related_blocking_compaction;
pub use compact::validate_compact_summary_for_testing;
pub use compact::CompactContextView;
pub use compact::CompactSummaryClampResult;

/// Manages agent sessions and their workflows.
///
/// Public facade for session runtime. Domain logic lives in focused modules:
/// - `crate::agent::lifecycle` — create, resume, recover, delete, status
/// - `crate::agent::workflow` / `llm` / `tools` — execution pipeline
/// - `session_manager::{approvals,channel,compact,execution_mode,message_injection,reload,reset}`
///
/// Workspace path resolution stays in `crate::session` (e.g. `resolve_session_workspace_dir`).
#[derive(Clone)]
pub struct AgentSessionManager {
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: AppHandle,
    proxy_manager: Arc<MCPServiceProxyManager>,
    session_repo: Arc<dyn SessionRepository>,
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
        // Initialize context registry with providers
        let mut registry = ContextRegistry::new();

        // Register time/location context provider (high priority)
        registry.register(Box::new(TimeLocationContextProvider::new()));

        log::info!(
            "✅ Context registry initialized with {} providers",
            registry.provider_count()
        );

        Self {
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
            proxy_manager,
            session_repo,
            context_registry: Arc::new(registry),
        }
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
            context_registry: self.context_registry.clone(),
        }
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        session_id: String,
        name: Option<String>,
        model: Option<String>,
        provider: Option<String>,
        agent_config: crate::agent::AgentConfig,
    ) -> Result<SessionMetadata, String> {
        self.create_session_with_repo(
            self.session_repo.clone(),
            session_id,
            name,
            model,
            provider,
            agent_config,
            crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
            None,
        )
        .await
    }

    /// Create or update a session with a specific repository
    #[allow(clippy::too_many_arguments)]
    pub async fn create_session_with_repo(
        &self,
        session_repo: Arc<dyn crate::repositories::SessionRepository>,
        session_id: String,
        name: Option<String>,
        model: Option<String>,
        provider: Option<String>,
        agent_config: crate::agent::AgentConfig,
        workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode,
        docker_config: Option<crate::models::workspace_isolation::DockerWorkspaceConfig>,
    ) -> Result<SessionMetadata, String> {
        crate::agent::lifecycle::create_session(crate::agent::lifecycle::CreateSessionParams {
            session_repo,
            active_sessions: self.active_sessions.clone(),
            proxy_manager: self.proxy_manager.clone(),
            app_handle: self.app_handle.clone(),
            context_registry: self.context_registry.clone(),
            session_id,
            name,
            model,
            provider,
            agent_config,
            workspace_isolation,
            docker_config,
        })
        .await
    }

    /// Update session configuration (model, provider, and/or assistant binding)
    pub async fn update_session_config(
        &self,
        session_id: String,
        model: Option<String>,
        provider: Option<String>,
        assistant_id: Option<String>,
        recursive: Option<bool>,
    ) -> Result<(), String> {
        crate::agent::lifecycle::update_session_config(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            &session_id,
            model,
            provider,
            assistant_id,
            recursive,
        )
        .await
    }

    /// Resume an existing session by loading it into active sessions
    pub async fn resume_session(&self, session_id: &str) -> Result<SessionMetadata, String> {
        let result = crate::agent::lifecycle::resume_session(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            self.context_registry.clone(),
            session_id,
        )
        .await?;

        approvals::reemit_pending_approvals_on_resume(self, session_id).await;

        Ok(result)
    }

    /// Start an agent workflow for a session
    pub async fn start_workflow(
        &self,
        session_id: String,
        user_message: Message,
    ) -> Result<(), String> {
        self.ensure_session_active(&session_id).await?;
        crate::agent::workflow::start_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
            user_message,
        )
        .await
    }

    /// Handle an LLM response from the frontend
    pub async fn handle_llm_response(
        &self,
        session_id: String,
        assistant_message: Message,
    ) -> Result<(), String> {
        crate::agent::llm::handle_llm_response(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
            assistant_message,
        )
        .await
    }

    pub async fn report_llm_streaming_issue(
        &self,
        report: crate::agent::llm::types::StreamingIssueReport,
    ) -> Result<crate::agent::llm::StreamingIssueOutcome, String> {
        crate::agent::llm::handle_streaming_issue(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            report,
        )
        .await
    }

    /// Get session metadata
    pub async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, String> {
        crate::agent::lifecycle::get_session(&self.session_repo, session_id).await
    }

    /// Get all sessions
    pub async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, String> {
        crate::agent::lifecycle::get_all_sessions(&self.session_repo).await
    }

    pub async fn list_sessions(
        &self,
        cursor: Option<SessionListCursor>,
        limit: u64,
        search: Option<&str>,
        bookmarked_only: bool,
        status: Option<&str>,
    ) -> Result<SessionListPage, String> {
        crate::agent::lifecycle::list_sessions(
            &self.session_repo,
            cursor,
            limit,
            search,
            bookmarked_only,
            status,
        )
        .await
    }

    pub async fn list_attention_sessions(&self) -> Result<Vec<SessionMetadata>, String> {
        crate::agent::lifecycle::list_attention_sessions(&self.session_repo).await
    }

    /// Recover sessions stuck in BUSY state after app crash/restart
    pub async fn recover_sessions(&self) -> Result<(), String> {
        crate::agent::lifecycle::recover_sessions(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            self.context_registry.clone(),
        )
        .await?;

        crate::services::docker_provisioning::recover_provisioning_sessions(
            &crate::services::docker_provisioning::DockerProvisioningDeps {
                session_repo: Arc::clone(&self.session_repo),
                active_sessions: Arc::clone(&self.active_sessions),
                proxy_manager: Arc::clone(&self.proxy_manager),
                app_handle: self.app_handle.clone(),
            },
        )
        .await
    }

    /// Pause a running workflow
    pub async fn pause_workflow(&self, session_id: String) -> Result<(), String> {
        crate::agent::workflow::pause_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Resume a paused workflow
    pub async fn resume_workflow(&self, session_id: String) -> Result<(), String> {
        self.ensure_session_active(&session_id).await?;
        crate::agent::workflow::resume_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Load messages from DB into in-memory cache
    pub async fn init_session_with_messages(&self, session_id: &str) -> Result<(), String> {
        crate::agent::lifecycle::init_session_with_messages(&self.active_sessions, session_id).await
    }

    /// Terminate a running workflow
    pub async fn terminate_session(&self, session_id: String) -> Result<(), String> {
        crate::agent::workflow::terminate_session(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Cancel a running workflow
    pub async fn cancel_workflow(
        &self,
        session_id: String,
    ) -> Result<crate::commands::agent_commands::CancelWorkflowResult, String> {
        crate::agent::workflow::cancel_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Inject messages into the session
    pub async fn inject_messages(
        &self,
        session_id: String,
        messages: Vec<Message>,
    ) -> Result<bool, String> {
        self.ensure_session_active(&session_id).await?;
        message_injection::inject_messages(self, session_id, messages).await
    }

    /// Append messages into session history without triggering a workflow or touching pending_queue
    pub async fn append_messages(
        &self,
        session_id: &str,
        messages: Vec<Message>,
    ) -> Result<(), String> {
        self.ensure_session_active(session_id).await?;
        crate::services::MessageService::append_messages_without_workflow(
            &self.active_sessions,
            &self.app_handle,
            session_id,
            messages,
        )
        .await
    }

    pub async fn get_pending_queue(&self, session_id: &str) -> Result<Vec<Message>, String> {
        self.ensure_session_active(session_id).await?;
        crate::agent::pending_queue::list_pending_messages(&self.active_sessions, session_id).await
    }

    pub async fn cancel_pending_prompt(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<bool, String> {
        self.ensure_session_active(session_id).await?;
        crate::agent::pending_queue::cancel_pending_message(
            &self.active_sessions,
            &self.app_handle,
            session_id,
            message_id,
        )
        .await
    }

    pub async fn ensure_session_active(&self, session_id: &str) -> Result<(), String> {
        let is_active = {
            let active = self.active_sessions.read().await;
            active.contains_key(session_id)
        };

        if is_active {
            return Ok(());
        }

        self.resume_session(session_id).await?;
        self.init_session_with_messages(session_id).await?;
        Ok(())
    }

    pub async fn inject_channel_notification(
        &self,
        session_id: String,
        server_name: String,
        notification: ChannelNotification,
    ) -> Result<(String, bool), String> {
        channel::inject_channel_notification(self, session_id, server_name, notification).await
    }

    pub async fn resolve_channel_notification_target(
        &self,
        server_name: &str,
    ) -> Result<ChannelRouteCandidate, String> {
        channel::resolve_channel_notification_target(self, server_name).await
    }

    pub async fn inject_channel_notification_auto(
        &self,
        server_name: String,
        notification: ChannelNotification,
    ) -> Result<(ChannelRouteCandidate, String, bool), String> {
        let target = self
            .resolve_channel_notification_target(&server_name)
            .await?;
        let (message_id, triggered) = self
            .inject_channel_notification(target.session_id.clone(), server_name, notification)
            .await?;

        Ok((target, message_id, triggered))
    }

    /// Handle tool execution result from frontend
    pub async fn handle_tool_result(
        &self,
        session_id: String,
        tool_call_id: String,
        result: crate::commands::agent_commands::ToolExecutionResult,
    ) -> Result<(), String> {
        crate::agent::workflow::continue_workflow_after_tool(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
            tool_call_id,
            result,
        )
        .await
    }

    /// Respond to a pending tool execution approval
    pub async fn respond_tool_approval(
        &self,
        session_id: &str,
        tool_call_id: &str,
        approved: bool,
    ) -> Result<(), String> {
        approvals::respond_tool_approval(self, session_id, tool_call_id, approved).await
    }

    pub async fn respond_channel_permission(
        &self,
        session_id: &str,
        request_id: &str,
        approved: bool,
    ) -> Result<String, String> {
        approvals::respond_channel_permission(self, session_id, request_id, approved).await
    }

    pub async fn get_execution_mode(&self, session_id: &str) -> ExecutionMode {
        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(session_id) {
                return session.execution_mode();
            }
        }

        if let Ok(Some(metadata)) = self.session_repo.get_session(session_id).await {
            return metadata.execution_mode;
        }

        ExecutionMode::Normal
    }

    pub async fn set_execution_mode(
        &self,
        session_id: &str,
        mode: ExecutionMode,
    ) -> Result<Vec<String>, String> {
        execution_mode::set_execution_mode(self, session_id, mode).await
    }

    /// Handle LLM error from frontend
    pub async fn handle_llm_error(
        &self,
        session_id: String,
        error: crate::agent::llm::types::AgentRuntimeError,
    ) -> Result<(), String> {
        crate::agent::llm::handle_llm_error(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            session_id,
            error,
        )
        .await
    }

    pub async fn get_session_display_name(&self, session_id: &str) -> Option<String> {
        let active = self.active_sessions.read().await;
        active.get(session_id).map(|session| {
            session
                .metadata
                .name
                .clone()
                .unwrap_or_else(|| session_id.chars().take(8).collect::<String>())
        })
    }

    /// Delete an agent session and cascade through its descendants.
    ///
    /// This removes the target session together with child sessions and their
    /// associated state, messages, workspace data, and search index entries.
    /// Use `delete_session_only` when descendants should remain as top-level
    /// sessions instead of being deleted.
    pub async fn delete_session(&self, session_id: String) -> Result<Vec<String>, String> {
        crate::agent::lifecycle::delete_session(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Delete only this session, leaving children as orphaned top-level sessions.
    pub async fn delete_session_only(
        &self,
        session_id: String,
    ) -> Result<(String, Vec<String>), String> {
        crate::agent::lifecycle::delete_session_only(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Get available tools for a session based on agent configuration.
    ///
    /// Returns the filtered tool list that matches what the LLM will receive.
    /// Wait for background tool loading... stdio/HTTP servers are spawned asynchronously...
    /// partial tool list is far better than no list.
    /// Soft-wait uses the configured MCP discovery timeout (default 30s).
    pub async fn get_available_tools(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
        self.proxy_manager
            .ensure_configured_proxy(session_id, Some(self.app_handle.clone()))
            .await?;

        let timeout_secs = self.proxy_manager.startup_timeout_secs().await;
        if let Err(e) = self
            .proxy_manager
            .wait_until_proxy_ready(session_id, timeout_secs)
            .await
        {
            log::warn!(
                "Proxy readiness wait failed for session '{}': {}",
                session_id,
                e
            );
        }

        crate::agent::tools::collect_available_tools(session_id, &self.proxy_manager).await
    }

    pub async fn get_runtime_state(
        &self,
        session_id: &str,
    ) -> crate::agent::runtime_state::SessionRuntimeState {
        self.proxy_manager.get_runtime_state(session_id).await
    }

    /// Get available tools for a session based on its config
    pub async fn get_tools_for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
        self.get_available_tools(session_id).await
    }

    /// Remove a message from the in-memory cache.
    /// Used when messages are deleted via messages_delete command to keep cache in sync.
    pub async fn remove_message_from_cache(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), String> {
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let mut messages = session.messages.write().await;
            messages.retain(|m| m.id != message_id);
            log::debug!(
                "Removed message {} from in-memory cache for session {}. Remaining: {}",
                message_id,
                session_id,
                messages.len()
            );
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Get compacted context for a session
    pub async fn get_compact_context(
        &self,
        session_id: &str,
    ) -> Result<Option<CompactContextRecord>, String> {
        compact::get_compact_context(&self.active_sessions, session_id).await
    }

    pub async fn get_compact_context_view(
        &self,
        session_id: &str,
    ) -> Result<Option<CompactContextView>, String> {
        compact::get_compact_context_view(&self.active_sessions, session_id).await
    }

    /// Save compacted context for a session
    pub async fn save_compact_context(
        &self,
        session_id: &str,
        record: CompactContextRecord,
    ) -> Result<(), String> {
        compact::save_compact_context(&self.active_sessions, session_id, record).await
    }

    /// Handle a successful compact response from the frontend.
    pub async fn handle_compact_response(
        &self,
        session_id: &str,
        to_id: String,
        compacted_delta_count: usize,
        summary: String,
    ) -> Result<compact::CompactResponseOutcome, String> {
        compact::handle_compact_response(compact::CompactResponseParams {
            active_sessions: &self.active_sessions,
            app_handle: &self.app_handle,
            session_repo: &self.session_repo,
            proxy_manager: &self.proxy_manager,
            session_id,
            to_id,
            compacted_delta_count,
            summary,
        })
        .await
    }

    /// Handle a compact error from the LLM service.
    pub async fn handle_compact_error(
        &self,
        session_id: String,
        error: crate::agent::llm::types::AgentRuntimeError,
    ) -> Result<(), String> {
        let dispatcher = TauriEventDispatcher::new(self.app_handle.clone());
        handle_compact_error_with_dispatcher(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            &dispatcher,
            session_id,
            error,
        )
        .await
    }

    pub async fn reset_session(&self, session_id: &str) -> Result<(), String> {
        reset::reset_session(self, session_id).await
    }

    /// Reload session tools, skills, and instructions context without wiping conversation history.
    ///
    /// Recreates the session MCP proxy to refresh tool schemas and assistant bindings,
    /// invalidates the in-memory prompt cache, and emits a session update event for frontend
    /// skills and tools revalidation.
    pub async fn reload_session(&self, session_id: &str) -> Result<(), String> {
        reload::reload_session(self, session_id).await
    }

    /// Trigger a non-resuming manual compaction pass for an already-active session.
    pub async fn trigger_manual_compaction(&self, session_id: &str) -> Result<bool, String> {
        crate::agent::llm::trigger_manual_compaction_for_session(
            &self.active_sessions,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Wait until a session is no longer compacting.
    pub async fn wait_for_compaction_to_settle(
        &self,
        session_id: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        compact::wait_for_compaction_to_settle(&self.active_sessions, session_id, timeout).await
    }
}
