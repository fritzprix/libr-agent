use crate::agent::channel_routing::ChannelRouteCandidate;
use crate::agent::concurrency::ActiveAgentPermit;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::context::time_location::TimeLocationContextProvider;
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::mcp::types::ChannelNotification;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::{
    CompactContextRecord, CompactContextRepository, SessionMetadata, SessionRepository,
    SessionStatus,
};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::RwLock;

#[path = "session_manager/approvals.rs"]
mod approvals;
#[path = "session_manager/channel.rs"]
mod channel;
#[path = "session_manager/compact.rs"]
mod compact;

pub use channel::format_channel_payload_for_test;
pub use compact::handle_compact_error_with_dispatcher;

/// Manages agent sessions and their workflows
///
/// This struct acts as a facade, delegating actual logic to specialized modules:
/// - `lifecycle`: Session creation, recovery, and state management
/// - `workflow`: Task execution flow (start, stop, pause, resume)
/// - `llm`: LLM interaction and response handling
/// - `tools`: Tool execution and result handling
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
    ///
    /// Used at startup to register a shared reference in the global `state`
    /// module so that builtin MCP tools can look up per-session cancellation
    /// tokens without going through Tauri managed state.
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
    /// This creates a new instance with shared Arc references
    pub fn clone_for_task(&self) -> Self {
        Self {
            active_sessions: self.active_sessions.clone(),
            app_handle: self.app_handle.clone(),
            proxy_manager: self.proxy_manager.clone(),
            session_repo: self.session_repo.clone(),
            context_registry: self.context_registry.clone(),
        }
    }

    /// Create a new session (wrapper around create_session_with_repo using internal repo)
    pub async fn create_session(
        &self,
        session_id: String,
        name: Option<String>,
        model: Option<String>,
        provider: Option<String>,
        agent_config: crate::agent::AgentConfig,
    ) -> Result<SessionMetadata, String> {
        // Use the internal persistent repository
        self.create_session_with_repo(
            self.session_repo.clone(),
            session_id,
            name,
            model,
            provider,
            agent_config,
        )
        .await
    }

    /// Create or update a session with a specific repository (for ephemeral vs persistent)
    pub async fn create_session_with_repo(
        &self,
        session_repo: Arc<dyn crate::repositories::SessionRepository>,
        session_id: String,
        name: Option<String>,
        model: Option<String>,
        provider: Option<String>,
        agent_config: crate::agent::AgentConfig,
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
        })
        .await
    }

    /// Update agent configuration for an existing session
    pub async fn update_session_config(
        &self,
        session_id: String,
        model: Option<String>,
        provider: Option<String>,
        agent_config: crate::agent::AgentConfig,
    ) -> Result<(), String> {
        crate::agent::lifecycle::update_session_config(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            &session_id,
            model,
            provider,
            agent_config,
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

        let pending_events = {
            let mut evs = Vec::new();
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(session_id) {
                let approvals = session.pending_approvals.read().await;
                for (tool_call_id, data) in approvals.iter() {
                    evs.push(
                        crate::agent::events::AgentEvent::ToolExecutionRequiresApproval {
                            session_id: session_id.to_string(),
                            tool_call_id: tool_call_id.clone(),
                            tool_name: data.tool_name.clone(),
                            arguments: data.arguments.clone(),
                        },
                    );
                    if let Some(request_id) = &data.request_id {
                        evs.push(crate::agent::events::AgentEvent::ChannelPermissionRequest {
                            session_id: session_id.to_string(),
                            request_id: request_id.clone(),
                            tool_call_id: tool_call_id.clone(),
                            tool_name: data.tool_name.clone(),
                            description: data.description.clone().unwrap_or_else(|| {
                                crate::agent::tool_approvals::build_channel_permission_description(
                                    &data.tool_name,
                                    &data.arguments,
                                )
                            }),
                            input_preview: data.input_preview.clone().unwrap_or_else(|| {
                                crate::agent::tool_approvals::build_channel_permission_input_preview(
                                    &data.arguments,
                                )
                            }),
                        });
                    }
                }
            }
            evs
        };

        // Emit the existing pending approvals
        for event in pending_events {
            if let Err(e) = crate::agent::tauri_events::emit_agent_event(&self.app_handle, event) {
                log::error!("Failed to re-emit pending approval event on resume: {}", e);
            }
        }

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
    ) -> Result<Option<crate::agent::llm::types::PostResponseCompactionPressure>, String> {
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

    /// Get session metadata
    pub async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, String> {
        crate::agent::lifecycle::get_session(&self.session_repo, session_id).await
    }

    /// Get all sessions
    pub async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, String> {
        crate::agent::lifecycle::get_all_sessions(&self.session_repo).await
    }

    /// Recover sessions stuck in BUSY state after app crash/restart
    pub async fn recover_sessions(&self) -> Result<(), String> {
        crate::agent::lifecycle::recover_sessions(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            self.context_registry.clone(),
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
    pub async fn cancel_workflow(&self, session_id: String) -> Result<(), String> {
        crate::agent::workflow::cancel_workflow(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Inject messages into the session and let the backend decide whether to
    /// continue the workflow based on the current session state.
    pub async fn inject_messages(
        &self,
        session_id: String,
        messages: Vec<Message>,
    ) -> Result<bool, String> {
        self.ensure_session_active(&session_id).await?;
        let should_trigger_workflow = {
            let sessions = self.active_sessions.read().await;
            let session = sessions
                .get(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            let is_transitioning_to_busy = matches!(
                session.status_transition.read().await.as_ref(),
                Some(crate::agent::state::SessionStatusTransition::ToStatus(
                    SessionStatus::Busy
                ))
            );

            session.metadata.status != SessionStatus::Busy && !is_transitioning_to_busy
        };

        // Delegate message persistence, caching, and event emission to MessageService
        crate::services::MessageService::inject_messages_to_session(
            &self.active_sessions,
            &self.app_handle,
            &session_id,
            messages,
            should_trigger_workflow,
        )
        .await?;

        if should_trigger_workflow {
            log::info!(
                "Triggering workflow after message injection for session: {}",
                session_id
            );

            {
                let sessions = self.active_sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    session.cancel_pending.store(false, Ordering::SeqCst);
                }
            }

            // Update status to Busy
            crate::agent::lifecycle::update_session_status(
                &self.session_repo,
                &self.active_sessions,
                &self.app_handle,
                &session_id,
                crate::repositories::SessionStatus::Busy,
            )
            .await?;

            // Emit workflow started event
            let event = crate::agent::events::AgentEvent::WorkflowStarted {
                session_id: session_id.clone(),
            };
            if let Err(e) = crate::agent::tauri_events::emit_agent_event(&self.app_handle, event) {
                log::error!(
                    "Failed to emit WorkflowStarted event during injection: {}",
                    e
                );
            }

            crate::agent::workflow::start::ensure_proxy_ready(
                &self.proxy_manager,
                &self.app_handle,
                &session_id,
                60,
            )
            .await?;

            crate::agent::llm::request_llm_completion(
                &self.session_repo,
                &self.active_sessions,
                &self.proxy_manager,
                &self.app_handle,
                session_id,
            )
            .await?;
        }

        Ok(should_trigger_workflow)
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
        // Use shared workflow logic for consistency between internal and external tool execution
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

    /// Set YOLO mode for a session
    pub async fn set_yolo_mode(&self, session_id: &str, enabled: bool) -> Result<(), String> {
        // 1. Update in-memory state
        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(session_id) {
                session
                    .yolo_mode
                    .store(enabled, std::sync::atomic::Ordering::Relaxed);
            }
        }

        // 2. Persist to DB via partial update
        self.session_repo
            .update_yolo_mode(session_id, enabled)
            .await
            .map_err(|e| format!("Failed to update session YOLO mode: {}", e))?;

        Ok(())
    }

    /// Returns the current yolo_mode for a session (false if session not found).
    pub async fn get_yolo_mode(&self, session_id: &str) -> bool {
        let active = self.active_sessions.read().await;
        active
            .get(session_id)
            .map(|s| s.yolo_mode.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
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

    /// Delete an agent session and all its data
    ///
    /// **Cascade Philosophy:** "When a parent is deleted, its children are also deleted"
    /// - DB-level CASCADE automatically deletes child session records
    /// - We must manually delete workspace directories for all descendants before DB deletion
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
    ///
    /// - Direct children have their `parent_session_id` set to NULL (become top-level)
    /// - Only this session's workspace and search index are removed
    /// - No cascade to descendants
    pub async fn delete_session_only(&self, session_id: String) -> Result<(), String> {
        crate::agent::lifecycle::delete_session_only(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
        )
        .await
    }

    /// Get available tools for a session based on agent configuration
    /// Returns the filtered tool list that matches what the LLM will receive
    pub async fn get_available_tools(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
        let active = self.active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let agent_config = session
            .metadata
            .agent_config
            .as_ref()
            .ok_or_else(|| "Agent configuration is required".to_string())
            .and_then(|json| {
                crate::agent::AgentConfig::from_json(json).map_err(|e| e.to_string())
            })?;

        drop(active); // Release the read lock before async call

        // Wait for background tool loading to finish (stdio/HTTP servers are spawned
        // asynchronously during proxy creation). Use a short timeout here (10 s) because
        // this is a UI query path — a partial tool list is far better than blocking the
        // status indicator for a full minute while slow/failing external servers are
        // still initialising. The LLM execution path uses the full 60 s timeout.
        if let Err(e) = self
            .proxy_manager
            .wait_until_proxy_ready(session_id, 10)
            .await
        {
            log::warn!(
                "Proxy readiness wait failed for session '{}': {}",
                session_id,
                e
            );
            // Continue anyway – partial tool list is better than no list.
        }

        // Use existing collect_available_tools function (same as LLM request)
        crate::agent::tools::collect_available_tools(session_id, &agent_config, &self.proxy_manager)
            .await
    }

    /// Get available tools for a session based on its config
    pub async fn get_tools_for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
        // 1. Get session config
        // Try active first
        let config = {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(session_id) {
                if let Some(config_str) = &session.metadata.agent_config {
                    crate::agent::AgentConfig::from_json(config_str)?
                } else {
                    return Err("Session has no config".to_string());
                }
            } else {
                // Try DB
                let session_opt =
                    crate::agent::lifecycle::get_session(&self.session_repo, session_id).await?;
                if let Some(session) = session_opt {
                    if let Some(config_str) = &session.agent_config {
                        crate::agent::AgentConfig::from_json(config_str)?
                    } else {
                        return Err("Session has no config".to_string());
                    }
                } else {
                    return Err("Session not found".to_string());
                }
            }
        };

        // 2. Call collect_available_tools
        crate::agent::tools::collect_available_tools(session_id, &config, &self.proxy_manager).await
    }

    /// Remove a message from the in-memory cache
    /// Used when messages are deleted via messages_delete command to keep cache in sync
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

    /// Get compacted context for a session (SP17)
    pub async fn get_compact_context(
        &self,
        session_id: &str,
    ) -> Result<Option<CompactContextRecord>, String> {
        let active = self.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let compact = session.compact_context.read().await;
            if compact.is_some() {
                return Ok((*compact).clone());
            }
        }

        // If not in active cache OR cache is None, check DB directly as safety measure
        let repo = crate::state::get_compact_context_repository();
        repo.get_by_session_id(session_id)
            .await
            .map_err(|e| e.to_string())
    }

    /// Save compacted context for a session (SP17)
    pub async fn save_compact_context(
        &self,
        session_id: &str,
        record: CompactContextRecord,
    ) -> Result<(), String> {
        // 1. Update in-memory if active
        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(session_id) {
                let mut compact = session.compact_context.write().await;
                *compact = Some(record.clone());
            }
        }

        // 2. Persist to DB
        let repo = crate::state::get_compact_context_repository();
        repo.upsert(&record).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Handle a successful compact response from the frontend.
    /// Stores the summary record in-memory + DB and clears the in-flight flag.
    pub async fn handle_compact_response(
        &self,
        session_id: &str,
        from_id: String,
        to_id: String,
        summary: String,
    ) -> Result<(), String> {
        compact::handle_compact_response(self, session_id, from_id, to_id, summary).await
    }

    /// Handle a compact error from the LLM service. If we were awaiting compaction, fail the workflow.
    pub async fn handle_compact_error(
        &self,
        session_id: String,
        error: crate::agent::llm::types::AgentRuntimeError,
    ) -> Result<(), String> {
        let dispatcher = TauriEventDispatcher::new(self.app_handle.clone());
        handle_compact_error_with_dispatcher(
            &self.session_repo,
            &self.active_sessions,
            &dispatcher,
            session_id,
            error,
        )
        .await
    }

    /// Clear the compact in-flight flag for a session (called on success or error).
    pub async fn clear_compact_in_flight(&self, session_id: &str) {
        crate::agent::compact_recovery::clear_compact_in_flight(&self.active_sessions, session_id)
            .await;
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
        let started_at = Instant::now();

        loop {
            let (compact_in_flight, awaiting_compact_completion) = {
                let active = self.active_sessions.read().await;
                let session = active
                    .get(session_id)
                    .ok_or_else(|| format!("Session not found: {}", session_id))?;
                (
                    session.compaction.in_flight.clone(),
                    session.compaction.awaiting_completion.clone(),
                )
            };

            if !compact_in_flight.load(Ordering::SeqCst)
                && !awaiting_compact_completion.load(Ordering::SeqCst)
            {
                return Ok(());
            }

            if started_at.elapsed() >= timeout {
                return Err(format!(
                    "Timed out waiting for compaction to settle for session {} after {} seconds",
                    session_id,
                    timeout.as_secs()
                ));
            }

            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }
}
