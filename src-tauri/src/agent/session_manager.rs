use crate::agent::state::AgentSession;
use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::SessionMetadata;
use std::collections::HashMap;
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
#[derive(Debug)]
pub struct AgentSessionManager {
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: AppHandle,
    proxy_manager: Arc<MCPServiceProxyManager>,
}

impl AgentSessionManager {
    /// Create a new AgentSessionManager
    pub fn new(app_handle: AppHandle, proxy_manager: Arc<MCPServiceProxyManager>) -> Self {
        Self {
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            app_handle,
            proxy_manager,
        }
    }

    /// Clone self for use in async tasks
    /// This creates a new instance with shared Arc references
    pub fn clone_for_task(&self) -> Self {
        Self {
            active_sessions: self.active_sessions.clone(),
            app_handle: self.app_handle.clone(),
            proxy_manager: self.proxy_manager.clone(),
        }
    }

    /// Create or update a session in the database
    pub async fn create_session(
        &self,
        session_id: String,
        name: Option<String>,
        agent_config: crate::agent::AgentConfig,
    ) -> Result<SessionMetadata, String> {
        crate::agent::lifecycle::create_session(
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
            session_id,
            name,
            agent_config,
        )
        .await
    }

    /// Update agent configuration for an existing session
    pub async fn update_session_config(
        &self,
        session_id: String,
        agent_config: crate::agent::AgentConfig,
    ) -> Result<(), String> {
        crate::agent::lifecycle::update_session_config(
            &self.active_sessions,
            &self.app_handle,
            &session_id,
            agent_config,
        )
        .await
    }

    /// Resume an existing session by loading it into active sessions
    #[allow(dead_code)]
    pub async fn resume_session(&self, session_id: &str) -> Result<SessionMetadata, String> {
        crate::agent::lifecycle::resume_session(
            &self.active_sessions,
            &self.proxy_manager,
            &self.app_handle,
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
        crate::agent::lifecycle::get_session(session_id).await
    }

    /// Get all sessions
    pub async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, String> {
        crate::agent::lifecycle::get_all_sessions().await
    }

    /// Recover sessions stuck in BUSY state after app crash/restart
    pub async fn recover_sessions(&self) -> Result<(), String> {
        crate::agent::lifecycle::recover_sessions(&self.active_sessions, &self.app_handle).await
    }

    /// Pause a running workflow
    pub async fn pause_workflow(&self, session_id: String) -> Result<(), String> {
        crate::agent::workflow::pause_workflow(&self.active_sessions, &self.app_handle, session_id)
            .await
    }

    /// Resume a paused workflow
    pub async fn resume_workflow(&self, session_id: String) -> Result<(), String> {
        crate::agent::workflow::resume_workflow(&self.active_sessions, &self.app_handle, session_id)
            .await
    }

    /// Load messages from DB into in-memory cache
    pub async fn init_session_with_messages(&self, session_id: &str) -> Result<(), String> {
        crate::agent::lifecycle::init_session_with_messages(&self.active_sessions, session_id).await
    }

    /// Terminate a running workflow
    pub async fn terminate_session(&self, session_id: String) -> Result<(), String> {
        crate::agent::workflow::terminate_session(
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

        // 2. Add messages to in-memory cache
        {
            let sessions = self.active_sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                let mut session_messages = session.messages.write().await;
                for msg in &messages {
                    session_messages.push(msg.clone());
                    if session_messages.len() > crate::agent::state::MAX_CACHED_MESSAGES {
                        session_messages.remove(0);
                    }
                }
            } else {
                return Err(format!("Session not found: {}", session_id));
            }
        }

        // 3. Emit MessageAdded events
        for msg in &messages {
            let event = crate::agent::events::AgentEvent::MessageAdded {
                session_id: session_id.clone(),
                message: Box::new(msg.clone()),
            };
            crate::agent::events::emit_agent_event(&self.app_handle, event)
                .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;
        }

        // 4. Persist to DB asynchronously
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

            // [Fix Option 1] Inline status update to ensure UI reflects 'Busy' state
            // 1. Update status to Busy
            crate::agent::lifecycle::update_session_status(
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

            // We use request_llm_completion directly here as we don't need the full start_workflow logic
            // (which assumes a User message as input)
            crate::agent::llm::request_llm_completion(
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
        // NOTE: This main entry point is for results coming from FRONTEND (if any)
        // or internal calls. However, for internal execution loop (in llm.rs), the logic
        // is handled by llm::handle_tool_result_and_continue.

        // If this is called from outside the loop (legacy path?), we might need to know if
        // we should continue.
        // For now, let's implement it by calling tools::handle_tool_result and LOGGING if logic stopped.
        // BUT, if it returns accumulated messages, we probably need to process them like llm.rs does.
        // Since all tool execution is now internal via `llm.rs` spawned tasks, this method
        // might only be used by `agent_commands.rs`.

        // Let's replicate the logic from `llm.rs` for completeness,
        // but assuming it's triggered externally.

        // We'll trust `llm.rs` to handle its own flow. If this is called externally,
        // we should probably use the same logic flow.
        // However, extracting `handle_tool_result_and_continue` from `llm.rs` into `workflow.rs` or `tools.rs`
        // would be cleaner to avoid duplication.
        // For now, to keep it simple and compile-safe, I will reference `tools::handle_tool_result`
        // and add a TODO or duplicate the continuation logic if needed.

        // Actually, `agent_commands.rs` calls this when frontend sends tool result.
        // But in our architecture, tool execution is internal (mostly).
        // If we support frontend-side tools, we need this.

        match crate::agent::tools::handle_tool_result(
            &self.active_sessions,
            &self.app_handle,
            session_id.clone(),
            tool_call_id,
            result,
        )
        .await
        {
            Ok(Some(accumulated_messages)) => {
                log::info!(
                    "External/Manual tool result completed turn for session {}. Proceeding.",
                    session_id
                );

                // Duplicate continuation logic (or move to shared func in pending refactor)
                // 1. Add to cache
                {
                    let sessions = self.active_sessions.read().await;
                    if let Some(session) = sessions.get(&session_id) {
                        let mut messages = session.messages.write().await;
                        for msg in &accumulated_messages {
                            messages.push(msg.clone());
                            if messages.len() > crate::agent::state::MAX_CACHED_MESSAGES {
                                messages.remove(0);
                            }
                        }
                    }
                }

                // 2. Emit MessageAdded
                for msg in &accumulated_messages {
                    let event = crate::agent::events::AgentEvent::MessageAdded {
                        session_id: session_id.clone(),
                        message: Box::new(msg.clone()),
                    };
                    let _ = crate::agent::events::emit_agent_event(&self.app_handle, event);
                }

                // 3. Persist to DB
                let msgs_clone = accumulated_messages.clone();

                tokio::spawn(async move {
                    let repo = crate::state::get_message_repository();
                    for msg in msgs_clone {
                        let _ = repo.insert(&msg).await;
                    }
                });

                // 4. Request LLM
                // (Skip UI detection for now or duplicate it)
                crate::agent::llm::request_llm_completion(
                    &self.active_sessions,
                    &self.proxy_manager,
                    &self.app_handle,
                    session_id,
                )
                .await?;
            }
            Ok(None) => {}
            Err(e) => return Err(e),
        }

        Ok(())
    }

    /// Handle LLM error from frontend
    pub async fn handle_llm_error(&self, session_id: String, error: String) -> Result<(), String> {
        crate::agent::llm::handle_llm_error(
            &self.active_sessions,
            &self.app_handle,
            session_id,
            error,
        )
        .await
    }

    /// Delete an agent session and all its data
    pub async fn delete_session(&self, session_id: String) -> Result<(), String> {
        use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
        use crate::repositories::session_repository::SessionRepository as SessionRepositoryTrait;

        // 1. Terminate workflow if running
        let _ = self.terminate_session(session_id.clone()).await;

        // 2. Remove from active sessions
        self.active_sessions.write().await.remove(&session_id);

        // 3. Delete all messages for the session
        let msg_repo = crate::state::get_message_repository();
        msg_repo
            .delete_by_session(&session_id)
            .await
            .map_err(|e| format!("Failed to delete messages: {}", e))?;

        // 4. Delete session metadata from database
        let session_repo = crate::state::get_session_repository();
        session_repo
            .delete_session(&session_id)
            .await
            .map_err(|e| format!("Failed to delete session metadata: {}", e))?;

        // 5. Delete search index
        if let Err(e) = crate::search::index_storage::delete_index(&session_id) {
            log::warn!(
                "Failed to delete search index for session {}: {}",
                session_id,
                e
            );
        }

        // 6. Delete index metadata
        if let Err(e) = session_repo.delete_index_metadata(&session_id).await {
            log::warn!(
                "Failed to delete index metadata for session {}: {}",
                session_id,
                e
            );
        }

        log::info!("✅ Deleted agent session: {}", session_id);
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

        // Use existing collect_available_tools function (same as LLM request)
        crate::agent::tools::collect_available_tools(session_id, &agent_config, &self.proxy_manager)
            .await
    }
}
