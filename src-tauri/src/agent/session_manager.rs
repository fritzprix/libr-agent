use crate::agent::context::registry::ContextRegistry;
use crate::agent::context::time_location::TimeLocationContextProvider;
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::{SessionMetadata, SessionRepository};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Manages agent sessions and their workflows
///
/// This struct acts as a facade, delegating actual logic to specialized modules:
/// - `lifecycle`: Session creation, recovery, and state management
/// - `workflow`: Task execution flow (start, stop, pause, resume)
/// - `llm`: LLM interaction and response handling
/// - `tools`: Tool execution and result handling
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
        crate::agent::lifecycle::resume_session(
            &self.session_repo,
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            self.context_registry.clone(),
            session_id,
        )
        .await
    }

    /// Start an agent workflow for a session
    pub async fn start_workflow(
        &self,
        session_id: String,
        user_message: Message,
    ) -> Result<(), String> {
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

    /// Inject messages into the session and optionally trigger the workflow
    pub async fn inject_messages(
        &self,
        session_id: String,
        messages: Vec<Message>,
        trigger_workflow: bool,
    ) -> Result<(), String> {
        // 1. Ensure cache is initialized
        crate::agent::lifecycle::ensure_cache_initialized(&self.active_sessions, &session_id)
            .await?;

        // 2. Get session reference (single lock acquisition)
        let sessions = self.active_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        // 3. Add messages to in-memory cache
        {
            let mut session_messages = session.messages.write().await;
            for msg in &messages {
                session_messages.push(msg.clone());
                if session_messages.len() > crate::agent::state::MAX_CACHED_MESSAGES {
                    session_messages.remove(0);
                }
            }
        }

        // 4. Emit MessageAdded events ONLY when triggering workflow
        // When triggerWorkflow=false, messages stay in backend cache without UI update
        // Frontend will add to pendingMessages queue and display with pending state
        if trigger_workflow {
            // Drop session lock before I/O operations
            drop(sessions);

            for msg in &messages {
                let event = crate::agent::events::AgentEvent::MessageAdded {
                    session_id: session_id.clone(),
                    message: Box::new(msg.clone()),
                };
                crate::agent::events::emit_agent_event(&self.app_handle, event)
                    .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;
            }
        } else {
            // Track these message IDs as pending (will emit when workflow picks them up)
            let mut pending_events = session.pending_events.write().await;
            for msg in &messages {
                pending_events.add(crate::agent::state::PendingEvent::Message(msg.id.clone()));
            }
            log::info!(
                "Marked {} messages as pending for session: {} (IDs: {:?})",
                messages.len(),
                session_id,
                messages.iter().map(|m| &m.id).collect::<Vec<_>>()
            );
        }

        // 5. Persist to DB asynchronously
        let msgs_for_db = messages.clone();
        tokio::spawn(async move {
            let repo = crate::state::get_message_repository();
            for msg in msgs_for_db {
                if let Err(e) = repo.insert(&msg).await {
                    log::error!("Failed to inject message to DB: {}", e);
                }
            }
        });

        // 5. Trigger workflow if requested
        if trigger_workflow {
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

            // [Fix Option 1] Inline status update to ensure UI reflects 'Busy' state
            // 1. Update status to Busy
            crate::agent::lifecycle::update_session_status(
                &self.session_repo,
                &self.active_sessions,
                &self.app_handle,
                &session_id,
                crate::repositories::SessionStatus::Busy,
            )
            .await?;

            // 2. Emit workflow started event
            let event = crate::agent::events::AgentEvent::WorkflowStarted {
                session_id: session_id.clone(),
            };
            if let Err(e) = crate::agent::events::emit_agent_event(&self.app_handle, event) {
                log::error!(
                    "Failed to emit WorkflowStarted event during injection: {}",
                    e
                );
            }

            // Wait for proxy to be ready before invoking LLM (mirrors start_workflow behaviour).
            // This is essential for freshly-created sessions (e.g. scheduled tasks) where the
            // MCPServiceProxy is still spawning external MCP servers asynchronously.
            if let Err(e) = self
                .proxy_manager
                .wait_until_proxy_ready(&session_id, 60)
                .await
            {
                log::warn!(
                    "Proxy readiness wait failed for session '{}': {}. Proceeding anyway.",
                    session_id,
                    e
                );
            }

            // We use request_llm_completion directly here as we don't need the full start_workflow logic
            // (which assumes a User message as input)
            crate::agent::llm::request_llm_completion(
                &self.session_repo,
                &self.active_sessions,
                &self.proxy_manager,
                &self.app_handle,
                session_id,
            )
            .await?;
        }

        Ok(())
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
        let active = self.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let mut approvals = session.pending_approvals.write().await;
            if let Some(sender) = approvals.remove(tool_call_id) {
                let _ = sender.send(approved);
                return Ok(());
            }
        }
        Err(format!(
            "Pending approval not found for tool call: {}",
            tool_call_id
        ))
    }

    /// Handle LLM error from frontend
    pub async fn handle_llm_error(&self, session_id: String, error: String) -> Result<(), String> {
        crate::agent::llm::handle_llm_error(
            &self.session_repo,
            &self.active_sessions,
            &self.app_handle,
            session_id,
            error,
        )
        .await
    }

    /// Delete an agent session and all its data
    ///
    /// **Cascade Philosophy:** "When a parent is deleted, its children are also deleted"
    /// - DB-level CASCADE automatically deletes child session records
    /// - We must manually delete workspace directories for all descendants before DB deletion
    pub async fn delete_session(&self, session_id: String) -> Result<(), String> {
        // 0. Collect all descendant IDs BEFORE cascade delete (so we can clean their workspaces)
        log::debug!(
            "Collecting descendants for cascade workspace cleanup: {}",
            session_id
        );
        let descendant_ids =
            crate::services::SessionCleanupService::collect_descendant_ids(&session_id).await?;

        if !descendant_ids.is_empty() {
            log::info!(
                "🌲 Cascade delete: {} will remove {} descendant session(s)",
                session_id,
                descendant_ids.len()
            );
        }

        // 1. Terminate workflow if running (for this session and all descendants)
        let _ = self.terminate_session(session_id.clone()).await;
        for descendant_id in &descendant_ids {
            let _ = self.terminate_session(descendant_id.clone()).await;
        }

        // 2. Remove from active sessions (parent + any loaded descendants)
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.remove(&session_id);
            for descendant_id in &descendant_ids {
                sessions.remove(descendant_id);
            }
        }

        // 3. Delete workspaces and DB cascade
        crate::services::SessionCleanupService::delete_session_data_cascade(
            &session_id,
            &descendant_ids,
        )
        .await?;

        log::info!(
            "✅ Deleted agent session: {} (cascade removed {} descendants)",
            session_id,
            descendant_ids.len()
        );
        Ok(())
    }

    /// Delete only this session, leaving children as orphaned top-level sessions.
    ///
    /// - Direct children have their `parent_session_id` set to NULL (become top-level)
    /// - Only this session's workspace and search index are removed
    /// - No cascade to descendants
    pub async fn delete_session_only(&self, session_id: String) -> Result<(), String> {
        // 1. Terminate workflow if running (this session only)
        let _ = self.terminate_session(session_id.clone()).await;

        // 2. Remove from active sessions map
        self.active_sessions.write().await.remove(&session_id);

        // 3. Delete workspace and db
        crate::services::SessionCleanupService::delete_session_data_only(&session_id).await?;

        log::info!(
            "✅ Deleted session only (children orphaned): {}",
            session_id
        );
        Ok(())
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
            // Session not active in memory - no cache to update (this is OK)
            log::debug!(
                "Session {} not active, skipping in-memory cache update for message deletion",
                session_id
            );
            Ok(())
        }
    }
}
