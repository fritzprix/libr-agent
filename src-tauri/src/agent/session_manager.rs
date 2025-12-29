use crate::agent::state::{AgentSession, PendingToolExecution, MAX_CACHED_MESSAGES};
use crate::agent::types::ToolCall;
use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::{MessageRepository, SessionMetadata, SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Manages agent sessions and their workflows
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
        let now = chrono::Utc::now().timestamp_millis();

        // Validate agent config
        agent_config.validate()?;

        // Serialize config for storage
        let config_json = agent_config.to_json()?;

        let session = SessionMetadata {
            id: session_id.clone(),
            name,
            status: SessionStatus::Idle,
            agent_config: Some(config_json),
            created_at: now,
            updated_at: now,
        };

        // Persist to database
        let session_repo = crate::state::get_session_repository();
        session_repo
            .upsert_session(&session)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;

        // Extract builtin tool IDs from agent config
        let tool_ids = crate::agent::tools::extract_builtin_tool_ids(&agent_config);

        // Create proxy for this session
        self.proxy_manager
            .create_proxy(session_id.clone(), tool_ids, Some(self.app_handle.clone()))
            .await?;

        log::info!(
            "Created MCP proxy for session: {} with builtin tools",
            session_id
        );

        // Add to active sessions with cancellation token and empty cache
        let mut active = self.active_sessions.write().await;
        active.insert(
            session_id.clone(),
            AgentSession {
                metadata: session.clone(),
                is_running: false,
                cancellation_token: CancellationToken::new(),
                pending_execution: None,
                messages: Arc::new(RwLock::new(Vec::new())),
                cache_initialized: Arc::new(AtomicBool::new(false)),
                last_synced_at: Arc::new(RwLock::new(None)),
            },
        );

        log::info!("Created agent session: {}", session_id);
        Ok(session)
    }

    /// Start an agent workflow for a session
    pub async fn start_workflow(
        &self,
        session_id: String,
        user_message: Message,
    ) -> Result<(), String> {
        // Check if workflow is cancelled before starting
        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(&session_id) {
                if session.cancellation_token.is_cancelled() {
                    return Err("Workflow was cancelled before starting".to_string());
                }
            }
        }

        // Update status to Busy
        self.update_session_status(&session_id, SessionStatus::Busy)
            .await?;

        // Emit workflow started event
        let event = crate::agent::events::AgentEvent::WorkflowStarted {
            session_id: session_id.clone(),
        };
        log::info!("Emitting WorkflowStarted event for session: {}", session_id);
        match crate::agent::events::emit_agent_event(&self.app_handle, event) {
            Ok(()) => log::info!("✅ WorkflowStarted event emitted successfully"),
            Err(e) => {
                log::error!("❌ Failed to emit WorkflowStarted event: {}", e);
                return Err(format!("Failed to emit event: {}", e));
            }
        }

        // Ensure cache is initialized before workflow
        self.ensure_cache_initialized(&session_id).await?;

        // 1. Add user message to in-memory cache FIRST (immediate, non-blocking)
        {
            let sessions = self.active_sessions.read().await;
            let session = sessions
                .get(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            let mut messages = session.messages.write().await;
            messages.push(user_message.clone());

            // Apply sliding window policy
            if messages.len() > MAX_CACHED_MESSAGES {
                let removed = messages.remove(0);
                log::debug!(
                    "Sliding window: evicted oldest message {} from session {}",
                    removed.id,
                    session_id
                );
            }

            log::info!(
                "📝 Message stack after user message: session={}, count={}, latest_message={}",
                session_id,
                messages.len(),
                user_message.id
            );
        } // Lock released

        // 2. Emit UI event (immediate)
        let message_added_event = crate::agent::events::AgentEvent::MessageAdded {
            session_id: session_id.clone(),
            message: Box::new(user_message.clone()),
        };
        crate::agent::events::emit_agent_event(&self.app_handle, message_added_event)
            .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;

        // 3. Persist to DB asynchronously (fire-and-forget)
        let msg_for_db = user_message.clone();
        let sid_for_db = session_id.clone();
        tokio::spawn(async move {
            let repo = crate::state::get_message_repository();
            if let Err(e) = repo.insert(&msg_for_db).await {
                log::error!(
                    "Failed to save user message to DB: session={}, msg_id={}, error={}",
                    sid_for_db,
                    msg_for_db.id,
                    e
                );
            }
        });

        log::info!(
            "Started workflow for session: {} with message: {}",
            session_id,
            user_message.id
        );

        // 4. Request LLM completion with cached messages (no DB query)
        self.request_llm_completion(session_id.clone()).await?;

        Ok(())
    }

    /// Handle an LLM response from the frontend
    pub async fn handle_llm_response(
        &self,
        session_id: String,
        assistant_message: Message,
    ) -> Result<(), String> {
        // Check if workflow is cancelled before processing response
        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(&session_id) {
                if session.cancellation_token.is_cancelled() {
                    log::info!("Workflow cancelled for session: {}", session_id);
                    return Err("Workflow was cancelled".to_string());
                }
            }
        }

        // 1. Add assistant message to cache
        {
            let sessions = self.active_sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                let mut messages = session.messages.write().await;
                messages.push(assistant_message.clone());

                // Apply sliding window
                if messages.len() > MAX_CACHED_MESSAGES {
                    let removed = messages.remove(0);
                    log::debug!("Sliding window: evicted message {}", removed.id);
                }

                log::info!(
                    "🤖 Message stack after assistant message: session={}, count={}, latest_message={}, has_tool_calls={}",
                    session_id,
                    messages.len(),
                    assistant_message.id,
                    assistant_message.tool_calls.is_some()
                );
            }
        }

        // 2. Emit MessageAdded event
        let message_added_event = crate::agent::events::AgentEvent::MessageAdded {
            session_id: session_id.clone(),
            message: Box::new(assistant_message.clone()),
        };
        crate::agent::events::emit_agent_event(&self.app_handle, message_added_event)
            .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;

        // 3. Persist to DB asynchronously
        let msg_for_db = assistant_message.clone();
        let sid_for_db = session_id.clone();
        tokio::spawn(async move {
            let repo = crate::state::get_message_repository();
            if let Err(e) = repo.insert(&msg_for_db).await {
                log::error!(
                    "Failed to save assistant message to DB: session={}, msg_id={}, error={}",
                    sid_for_db,
                    msg_for_db.id,
                    e
                );
            }
        });

        // Parse tool calls if present (now directly available as Vec<ToolCall>)
        let tool_calls: Vec<ToolCall> = if let Some(tool_calls_vec) = &assistant_message.tool_calls
        {
            log::debug!(
                "Processing {} tool calls for session {}",
                tool_calls_vec.len(),
                session_id
            );
            tool_calls_vec.clone()
        } else {
            log::info!(
                "No tool calls in assistant message for session {}",
                session_id
            );
            Vec::new()
        };

        log::info!(
            "Tool call processing for session {}: {} tool calls found",
            session_id,
            tool_calls.len()
        );

        if tool_calls.is_empty() {
            // No tools to execute, workflow is complete for this turn
            self.update_session_status(&session_id, SessionStatus::Idle)
                .await?;

            let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                session_id: session_id.clone(),
            };
            crate::agent::events::emit_agent_event(&self.app_handle, event)
                .map_err(|e| format!("Failed to emit event: {}", e))?;

            log::info!("Completed workflow for session: {}", session_id);
        } else {
            // Tools found! Initiate execution
            log::info!(
                "Processing {} tool calls for session: {}",
                tool_calls.len(),
                session_id
            );

            // Initialize pending execution state
            {
                let mut active = self.active_sessions.write().await;
                if let Some(session) = active.get_mut(&session_id) {
                    session.pending_execution = Some(PendingToolExecution {
                        total_expected: tool_calls.len(),
                        results: Vec::new(),
                        tool_names: tool_calls
                            .iter()
                            .map(|tc| (tc.id.clone(), tc.function.name.clone()))
                            .collect(),
                    });
                }
            }

            // Execute tool calls (all handled in Rust via proxy)
            for tool_call in tool_calls {
                let tool_name = &tool_call.function.name;

                // Emit ToolExecutionStarted event for UI progress tracking
                let event = crate::agent::events::AgentEvent::ToolExecutionStarted {
                    session_id: session_id.clone(),
                    tool_name: tool_name.clone(),
                };
                crate::agent::events::emit_agent_event(&self.app_handle, event)
                    .map_err(|e| format!("Failed to emit tool execution started event: {}", e))?;

                // Execute directly via proxy_manager (handles both builtin and external)
                log::info!(
                    "Executing tool '{}' via proxy_manager for session: {}",
                    tool_name,
                    session_id
                );

                // Execute the tool and handle result directly
                let tool_call_id = tool_call.id.clone();
                let args_str = tool_call.function.arguments.clone();
                let session_id_clone = session_id.clone();
                let proxy_manager = self.proxy_manager.clone();
                let tool_name_owned = tool_name.to_string();

                // Spawn async task to execute tool and process result
                let manager_ref = self.clone_for_task();
                tokio::spawn(async move {
                    // Parse arguments JSON string to Value
                    let args = match serde_json::from_str::<serde_json::Value>(&args_str) {
                        Ok(v) => v,
                        Err(e) => {
                            log::error!(
                                "Failed to parse tool arguments for session {}: {}",
                                session_id_clone,
                                e
                            );

                            let result = crate::commands::agent_commands::ToolExecutionResult {
                                success: false,
                                content: String::new(),
                                error: Some(format!("Failed to parse tool arguments: {}", e)),
                                is_error: true,
                                mcp_content: None,
                            };

                            if let Err(err) = manager_ref
                                .handle_tool_result(session_id_clone.clone(), tool_call_id, result)
                                .await
                            {
                                log::error!(
                                    "Failed to handle argument parse error for session {}: {}",
                                    session_id_clone,
                                    err
                                );
                            }
                            return;
                        }
                    };

                    match proxy_manager
                        .call_tool(&session_id_clone, &tool_name_owned, args)
                        .await
                    {
                        Ok(response) => {
                            // Convert MCPResponse to tool result
                            let content = response
                                .result
                                .as_ref()
                                .and_then(|r| serde_json::to_string_pretty(r).ok())
                                .unwrap_or_else(|| "{}".to_string());

                            let is_error = response.error.is_some();
                            let error_msg = response.error.map(|e| e.message);

                            let result = crate::commands::agent_commands::ToolExecutionResult {
                                success: !is_error,
                                content,
                                error: error_msg,
                                is_error,
                                mcp_content: crate::agent::tools::convert_mcp_response_content(
                                    response.result,
                                ),
                            };

                            if let Err(e) = manager_ref
                                .handle_tool_result(session_id_clone.clone(), tool_call_id, result)
                                .await
                            {
                                log::error!(
                                    "Failed to handle tool result for session {}: {}",
                                    session_id_clone,
                                    e
                                );
                            }

                            // Emit ToolExecutionCompleted event (success case)
                            let event = crate::agent::events::AgentEvent::ToolExecutionCompleted {
                                session_id: session_id_clone.clone(),
                                tool_name: tool_name_owned.clone(),
                                success: true,
                            };
                            let _ = crate::agent::events::emit_agent_event(
                                &manager_ref.app_handle,
                                event,
                            );
                        }
                        Err(e) => {
                            log::error!(
                                "Tool execution failed for session {}: {}",
                                session_id_clone,
                                e
                            );

                            let result = crate::commands::agent_commands::ToolExecutionResult {
                                success: false,
                                content: String::new(),
                                error: Some(e.clone()),
                                is_error: true,
                                mcp_content: None,
                            };

                            if let Err(err) = manager_ref
                                .handle_tool_result(session_id_clone.clone(), tool_call_id, result)
                                .await
                            {
                                log::error!(
                                    "Failed to handle tool error for session {}: {}",
                                    session_id_clone,
                                    err
                                );
                            }

                            // Emit ToolExecutionCompleted event (error case)
                            let event = crate::agent::events::AgentEvent::ToolExecutionCompleted {
                                session_id: session_id_clone.clone(),
                                tool_name: tool_name_owned.clone(),
                                success: false,
                            };
                            let _ = crate::agent::events::emit_agent_event(
                                &manager_ref.app_handle,
                                event,
                            );
                        }
                    }
                });
            }
        }

        Ok(())
    }

    /// Update session status in database and emit event
    async fn update_session_status(
        &self,
        session_id: &str,
        status: SessionStatus,
    ) -> Result<(), String> {
        let session_repo = crate::state::get_session_repository();
        session_repo
            .update_status(session_id, status.clone())
            .await
            .map_err(|e| format!("Failed to update session status: {}", e))?;

        // Update in-memory state
        let mut active = self.active_sessions.write().await;
        if let Some(session) = active.get_mut(session_id) {
            session.metadata.status = status.clone();
        }

        // Emit status changed event
        let event = crate::agent::events::AgentEvent::StatusChanged {
            session_id: session_id.to_string(),
            status,
        };
        crate::agent::events::emit_agent_event(&self.app_handle, event)
            .map_err(|e| format!("Failed to emit event: {}", e))?;

        Ok(())
    }

    /// Get session metadata
    pub async fn get_session(&self, session_id: &str) -> Result<Option<SessionMetadata>, String> {
        let session_repo = crate::state::get_session_repository();
        session_repo
            .get_session(session_id)
            .await
            .map_err(|e| format!("Failed to get session: {}", e))
    }

    /// Get all sessions
    pub async fn get_all_sessions(&self) -> Result<Vec<SessionMetadata>, String> {
        let session_repo = crate::state::get_session_repository();
        session_repo
            .get_all_sessions()
            .await
            .map_err(|e| format!("Failed to get all sessions: {}", e))
    }

    /// Recover sessions stuck in BUSY state after app crash/restart
    ///
    /// Called during app initialization to clean up sessions that were
    /// running when the application terminated unexpectedly.
    pub async fn recover_sessions(&self) -> Result<(), String> {
        log::info!("Starting session recovery process...");

        let session_repo = crate::state::get_session_repository();
        let all_sessions = session_repo
            .get_all_sessions()
            .await
            .map_err(|e| format!("Failed to query sessions for recovery: {}", e))?;

        let mut recovered_count = 0;

        for session in all_sessions {
            // Only recover sessions that were BUSY (actively running)
            if matches!(session.status, SessionStatus::Busy) {
                log::warn!(
                    "Recovering session '{}' from BUSY state (possible crash)",
                    session.id
                );

                // Reset to PAUSED (user can manually resume)
                self.update_session_status(&session.id, SessionStatus::Paused)
                    .await?;

                // Initialize session in active_sessions map with fresh state
                let mut active = self.active_sessions.write().await;
                active.insert(
                    session.id.clone(),
                    AgentSession {
                        metadata: session.clone(),
                        is_running: false,
                        cancellation_token: CancellationToken::new(),
                        pending_execution: None,
                        messages: Arc::new(RwLock::new(Vec::new())),
                        cache_initialized: Arc::new(AtomicBool::new(false)),
                        last_synced_at: Arc::new(RwLock::new(None)),
                    },
                );
                drop(active); // Release lock early

                recovered_count += 1;
            }
        }

        if recovered_count > 0 {
            log::info!(
                "Session recovery complete: {} session(s) recovered",
                recovered_count
            );
        } else {
            log::info!("Session recovery complete: No sessions to recover");
        }

        Ok(())
    }

    /// Pause a running workflow
    pub async fn pause_workflow(&self, session_id: String) -> Result<(), String> {
        self.update_session_status(&session_id, SessionStatus::Paused)
            .await?;

        let mut active = self.active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            session.is_running = false;
        }

        log::info!("Paused workflow for session: {}", session_id);
        Ok(())
    }

    /// Resume a paused workflow
    pub async fn resume_workflow(&self, session_id: String) -> Result<(), String> {
        self.update_session_status(&session_id, SessionStatus::Busy)
            .await?;

        let mut active = self.active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            session.is_running = true;
        }

        log::info!("Resumed workflow for session: {}", session_id);
        Ok(())
    }

    /// Load messages from DB into in-memory cache (called once per session)
    ///
    /// This method should be called:
    /// 1. When resuming an existing session
    /// 2. When starting a workflow for a session with no cached messages
    ///
    /// # Performance
    /// - Single DB query (page size 1000)
    /// - O(n) memory allocation for Vec<Message>
    /// - Runs synchronously to ensure cache ready before workflow starts
    pub async fn init_session_with_messages(&self, session_id: &str) -> Result<(), String> {
        let message_repo = crate::state::get_message_repository();

        // Load last 1000 messages from DB (one-time operation)
        let page = message_repo
            .get_page(session_id, 1, MAX_CACHED_MESSAGES)
            .await
            .map_err(|e| format!("Failed to load messages for session {}: {}", session_id, e))?;

        let loaded_count = page.items.len();

        // Populate in-memory cache
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let mut messages = session.messages.write().await;
            *messages = page.items; // Replace with DB data

            let mut synced_at = session.last_synced_at.write().await;
            *synced_at = Some(SystemTime::now());

            session.cache_initialized.store(true, Ordering::Release);

            log::info!(
                "Initialized session cache: session={}, messages_loaded={}",
                session_id,
                loaded_count
            );
        } else {
            return Err(format!("Session not found: {}", session_id));
        }

        Ok(())
    }

    /// Ensure cache is initialized before workflow starts (lazy initialization)
    async fn ensure_cache_initialized(&self, session_id: &str) -> Result<(), String> {
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            if !session.cache_initialized.load(Ordering::Acquire) {
                drop(sessions); // Release read lock before calling init
                self.init_session_with_messages(session_id).await?;
            }
        }
        Ok(())
    }

    /// Terminate a running workflow
    /// This triggers the cancellation token to abort any running operations
    pub async fn terminate_session(&self, session_id: String) -> Result<(), String> {
        log::info!("Terminating workflow for session: {}", session_id);

        // Trigger cancellation token to abort running loops
        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(&session_id) {
                session.cancellation_token.cancel();
            } else {
                return Err(format!("Session not found: {}", session_id));
            }
        }

        // Update status to idle (workflow stopped)
        self.update_session_status(&session_id, SessionStatus::Idle)
            .await?;

        // Destroy proxy for this session
        self.proxy_manager.destroy_proxy(&session_id).await;
        log::info!("Destroyed MCP proxy for session: {}", session_id);

        // Remove from active sessions and create a new cancellation token for future use
        let mut active = self.active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            session.is_running = false;
            // Reset cancellation token for potential future workflows
            session.cancellation_token = CancellationToken::new();
        }

        // Emit workflow stopped event
        let event = crate::agent::events::AgentEvent::WorkflowCompleted {
            session_id: session_id.clone(),
        };
        crate::agent::events::emit_agent_event(&self.app_handle, event)
            .map_err(|e| format!("Failed to emit event: {}", e))?;

        log::info!("Terminated workflow for session: {}", session_id);
        Ok(())
    }

    /// Handle tool execution result from frontend
    pub async fn handle_tool_result(
        &self,
        session_id: String,
        tool_call_id: String,
        result: crate::commands::agent_commands::ToolExecutionResult,
    ) -> Result<(), String> {
        // Check cancellation
        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(&session_id) {
                if session.cancellation_token.is_cancelled() {
                    log::info!("Workflow cancelled for session: {}", session_id);
                    return Err("Workflow was cancelled".to_string());
                }
            }
        }

        log::debug!(
            "Tool result received for session {}, tool_call_id: {}",
            session_id,
            tool_call_id
        );

        let mut should_continue = false;
        let mut accumulated_messages = Vec::new();

        // Scope to hold the write lock
        {
            let mut active = self.active_sessions.write().await;
            if let Some(session) = active.get_mut(&session_id) {
                if let Some(pending) = &mut session.pending_execution {
                    // Create Tool Message using helper methods
                    let message = if result.is_error {
                        crate::agent::tools::create_error_tool_result(
                            &session_id,
                            &tool_call_id,
                            result.error.as_deref().unwrap_or("Unknown error"),
                        )
                    } else if let Some(mcp_content) = result.mcp_content {
                        crate::agent::tools::create_tool_result_message_with_content(
                            &session_id,
                            &tool_call_id,
                            mcp_content,
                        )
                    } else {
                        crate::agent::tools::create_tool_result_message(
                            &session_id,
                            &tool_call_id,
                            result.content.clone(),
                        )
                    };

                    pending.results.push(message);

                    // Emit ToolExecutionCompleted event for external tools
                    if let Some(tool_name) = pending.tool_names.get(&tool_call_id) {
                        let event = crate::agent::events::AgentEvent::ToolExecutionCompleted {
                            session_id: session_id.clone(),
                            tool_name: tool_name.clone(),
                            success: !result.is_error,
                        };
                        let _ = crate::agent::events::emit_agent_event(&self.app_handle, event);
                    }

                    log::debug!(
                        "Accumulated result {}/{} for session {}",
                        pending.results.len(),
                        pending.total_expected,
                        session_id
                    );

                    // Check if all results are in
                    if pending.results.len() >= pending.total_expected {
                        should_continue = true;
                        // Move results out of pending state
                        accumulated_messages = pending.results.drain(..).collect();
                        // Clear pending state
                        session.pending_execution = None;
                    }
                } else {
                    log::warn!(
                        "Received tool result for session {} but no pending execution state found",
                        session_id
                    );
                    return Ok(()); // Ignore or error? Safe to ignore to prevent crashes
                }
            } else {
                return Err(format!("Session not found: {}", session_id));
            }
        }

        // If we collected all results, proceed to next step
        if should_continue {
            log::info!(
                "All tool results received for session {}. Proceeding to next step.",
                session_id
            );

            // 1. Add tool result messages to in-memory cache
            {
                let sessions = self.active_sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    let mut messages = session.messages.write().await;

                    log::info!(
                        "Adding {} tool result messages to cache for session {}",
                        accumulated_messages.len(),
                        session_id
                    );

                    for (idx, msg) in accumulated_messages.iter().enumerate() {
                        log::debug!(
                            "Tool result {}/{}: id={}, role={}, tool_call_id={:?}",
                            idx + 1,
                            accumulated_messages.len(),
                            msg.id,
                            msg.role,
                            msg.tool_call_id
                        );

                        messages.push(msg.clone());

                        // Apply sliding window per message
                        if messages.len() > MAX_CACHED_MESSAGES {
                            let removed = messages.remove(0);
                            log::debug!(
                                "Sliding window: evicted message {} from session {}",
                                removed.id,
                                session_id
                            );
                        }
                    }

                    log::info!(
                        "Cache updated: session {} now has {} total messages",
                        session_id,
                        messages.len()
                    );
                }
            }

            // 2. Emit UI events for each tool result
            log::info!(
                "Emitting {} MessageAdded events for tool results in session {}",
                accumulated_messages.len(),
                session_id
            );

            for (idx, msg) in accumulated_messages.iter().enumerate() {
                log::debug!(
                    "Creating MessageAdded event {}/{}: msg_id={}",
                    idx + 1,
                    accumulated_messages.len(),
                    msg.id
                );

                let event = crate::agent::events::AgentEvent::MessageAdded {
                    session_id: session_id.clone(),
                    message: Box::new(msg.clone()),
                };

                match crate::agent::events::emit_agent_event(&self.app_handle, event) {
                    Ok(()) => {
                        log::info!(
                            "✅ MessageAdded event emitted successfully: msg_id={}, tool_call_id={:?}",
                            msg.id,
                            msg.tool_call_id
                        );
                    }
                    Err(e) => {
                        log::error!(
                            "❌ Failed to emit MessageAdded event: msg_id={}, error={}",
                            msg.id,
                            e
                        );
                    }
                }
            }

            // 3. Persist to DB asynchronously (bulk insert)
            let msgs_for_db = accumulated_messages.clone();
            let sid_for_db = session_id.clone();
            log::info!(
                "Spawning async task to persist {} tool result messages to DB for session {}",
                msgs_for_db.len(),
                sid_for_db
            );

            tokio::spawn(async move {
                let repo = crate::state::get_message_repository();
                let mut success_count = 0;
                let mut error_count = 0;

                for msg in msgs_for_db {
                    match repo.insert(&msg).await {
                        Ok(()) => {
                            success_count += 1;
                            log::debug!(
                                "Tool result persisted to DB: session={}, msg_id={}",
                                sid_for_db,
                                msg.id
                            );
                        }
                        Err(e) => {
                            error_count += 1;
                            log::error!(
                                "Failed to save tool result to DB: session={}, msg_id={}, error={}",
                                sid_for_db,
                                msg.id,
                                e
                            );
                        }
                    }
                }

                log::info!(
                    "DB persistence complete for session {}: {} succeeded, {} failed",
                    sid_for_db,
                    success_count,
                    error_count
                );
            });

            // 4. Check for UI interaction requests (stop condition)
            // Optimization: We check only the *newly produced* tool results (accumulated_messages)
            // rather than iterating the entire session history. If any new result contains a Resource
            // (which implies a UI component in our protocol), we pause the workflow.
            let has_ui_interaction = accumulated_messages.iter().any(|msg| {
                msg.content
                    .iter()
                    .any(|c| matches!(c, crate::agent::types::MCPContent::Resource { .. }))
            });

            if has_ui_interaction {
                log::info!(
                    "UI interaction detected for session {}. Stopping recursive LLM loop.",
                    session_id
                );

                // Workflow is effectively "paused" or "completed" from the LLM's perspective
                // The UI will re-trigger the workflow when the user responds (e.g. via reply_prompt)
                self.update_session_status(&session_id, SessionStatus::Idle)
                    .await?;

                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.clone(),
                };
                crate::agent::events::emit_agent_event(&self.app_handle, event)
                    .map_err(|e| format!("Failed to emit event: {}", e))?;
            } else {
                // 5. Request next LLM completion with updated cache
                // The LLM will see the assistant's tool calls and the subsequent tool results
                self.request_llm_completion(session_id).await?;
            }
        }

        Ok(())
    }

    /// Handle LLM error from frontend
    pub async fn handle_llm_error(&self, session_id: String, error: String) -> Result<(), String> {
        log::error!("LLM error for session {}: {}", session_id, error);

        // Update session status to error
        self.update_session_status(&session_id, SessionStatus::Idle)
            .await?;

        // Emit error event
        let event = crate::agent::events::AgentEvent::WorkflowError {
            session_id: session_id.clone(),
            error: error.clone(),
        };
        crate::agent::events::emit_agent_event(&self.app_handle, event)
            .map_err(|e| format!("Failed to emit error event: {}", e))?;

        Ok(())
    }

    /// Build complete system prompt for session
    async fn build_system_prompt(&self, session_id: &str) -> Result<String, String> {
        // 1. Load Agent config
        let active = self.active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let agent_config = session
            .metadata
            .agent_config
            .as_ref()
            .ok_or_else(|| "Agent configuration is required but not found".to_string())
            .and_then(|json| {
                crate::agent::AgentConfig::from_json(json).map_err(|e| e.to_string())
            })?;

        let config_clone = agent_config.clone();
        drop(active);

        // 2. Get Service Proxy
        let proxy = self.proxy_manager.get_proxy(session_id).await;

        // 3. Build prompt using helper
        crate::agent::llm::build_system_prompt(&config_clone, proxy).await
    }

    /// Collect available tools for a session based on agent configuration
    async fn collect_available_tools(
        &self,
        session_id: &str,
        agent_config: &crate::agent::AgentConfig,
    ) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
        crate::agent::tools::collect_available_tools(session_id, agent_config, &self.proxy_manager)
            .await
    }

    /// Request LLM completion from frontend
    async fn request_llm_completion(&self, session_id: String) -> Result<(), String> {
        use tauri::Emitter;

        // ✅ Read messages from in-memory cache (zero DB query)
        let messages = {
            let sessions = self.active_sessions.read().await;
            let session = sessions
                .get(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            let messages_lock = session.messages.read().await;
            messages_lock.clone() // Clone to release lock quickly
        };

        log::info!(
            "🔄 Message stack for LLM request: session={}, count={}, first_msg_id={}, last_msg_id={}",
            session_id,
            messages.len(),
            messages.first().map(|m| m.id.as_str()).unwrap_or("none"),
            messages.last().map(|m| m.id.as_str()).unwrap_or("none")
        );

        // Get agent config from session metadata (REQUIRED - no fallback)
        let active = self.active_sessions.read().await;
        let session = active
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let agent_config = session
            .metadata
            .agent_config
            .as_ref()
            .ok_or_else(|| "Agent configuration is required but not found".to_string())
            .and_then(|json| crate::agent::AgentConfig::from_json(json))?;

        // Clone agent_config BEFORE extracting fields to avoid partial move
        let agent_config_clone = agent_config.clone();

        let model = agent_config.model;
        let provider = agent_config.provider;
        let temperature = Some(agent_config.temperature);
        let max_tokens = agent_config.max_tokens;

        drop(active); // Release read lock before emitting event

        // Build complete system prompt (agent base + service contexts)
        let system_prompt = Some(self.build_system_prompt(&session_id).await?);

        // Collect available tools (filtered by agent configuration)
        let available_tools = self
            .collect_available_tools(&session_id, &agent_config_clone)
            .await
            .ok();

        // Emit llm:completion-request event to frontend
        #[derive(Clone, serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CompletionRequest {
            session_id: String,
            messages: Vec<Message>,
            model: String,
            provider: String,
            system_prompt: Option<String>,
            temperature: Option<f32>,
            max_tokens: Option<u32>,
            available_tools: Option<Vec<crate::mcp::types::MCPTool>>,
        }

        let request = CompletionRequest {
            session_id: session_id.clone(),
            messages,
            model,
            provider,
            system_prompt,
            temperature,
            max_tokens,
            available_tools,
        };

        self.app_handle
            .emit("llm:completion-request", request)
            .map_err(|e| format!("Failed to emit LLM completion request: {}", e))?;

        log::info!("Emitted LLM completion request for session: {}", session_id);

        Ok(())
    }
}
