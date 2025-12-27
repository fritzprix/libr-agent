use crate::commands::messages_commands::Message;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::{MessageRepository, SessionMetadata, SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

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
}

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
        let tool_ids = extract_builtin_tool_ids(&agent_config);

        // Create proxy for this session
        self.proxy_manager
            .create_proxy(session_id.clone(), tool_ids)
            .await?;

        log::info!(
            "Created MCP proxy for session: {} with builtin tools",
            session_id
        );

        // Add to active sessions with cancellation token
        let mut active = self.active_sessions.write().await;
        active.insert(
            session_id.clone(),
            AgentSession {
                metadata: session.clone(),
                is_running: false,
                cancellation_token: CancellationToken::new(),
                pending_execution: None,
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
        crate::agent::events::emit_agent_event(&self.app_handle, event)
            .map_err(|e| format!("Failed to emit event: {}", e))?;

        // Save user message to database
        let message_repo = crate::state::get_message_repository();
        message_repo
            .insert(&user_message)
            .await
            .map_err(|e| format!("Failed to save message: {}", e))?;

        log::info!(
            "Started workflow for session: {} with message: {}",
            session_id,
            user_message.id
        );

        // Emit LLM completion request to frontend (LLMServiceProvider)
        // The frontend will handle the LLM streaming and send back the response via agent_handle_llm_response
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

        // Save assistant message to database
        let message_repo = crate::state::get_message_repository();
        message_repo
            .insert(&assistant_message)
            .await
            .map_err(|e| format!("Failed to save assistant message: {}", e))?;

        // Parse tool calls if present
        let tool_calls: Vec<crate::agent::types::ToolCall> =
            if let Some(tool_calls_json) = &assistant_message.tool_calls {
                serde_json::from_str(tool_calls_json)
                    .map_err(|e| format!("Failed to parse tool calls: {}", e))?
            } else {
                Vec::new()
            };

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

            // Dispatch execution requests to frontend
            // Using tauri::Emitter trait
            use tauri::Emitter;

            #[derive(Clone, serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct ToolExecutionRequest {
                session_id: String,
                tool_call: crate::agent::types::ToolCall,
            }

            // Execute tool calls (builtin handled in Rust, external emitted to frontend)
            for tool_call in tool_calls {
                let tool_name = &tool_call.function.name;

                // Emit ToolExecutionStarted event for UI progress tracking
                let event = crate::agent::events::AgentEvent::ToolExecutionStarted {
                    session_id: session_id.clone(),
                    tool_name: tool_name.clone(),
                };
                crate::agent::events::emit_agent_event(&self.app_handle, event)
                    .map_err(|e| format!("Failed to emit tool execution started event: {}", e))?;

                if tool_name.starts_with("builtin_") {
                    // Builtin tool - execute directly via proxy_manager
                    log::info!(
                        "Executing builtin tool '{}' via proxy_manager for session: {}",
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
                                };

                                if let Err(err) = manager_ref
                                    .handle_tool_result(
                                        session_id_clone.clone(),
                                        tool_call_id,
                                        result,
                                    )
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
                                    .and_then(|r| serde_json::to_string_pretty(&r).ok())
                                    .unwrap_or_else(|| "{}".to_string());

                                let is_error = response.error.is_some();
                                let error_msg = response.error.map(|e| e.message);

                                let result = crate::commands::agent_commands::ToolExecutionResult {
                                    success: !is_error,
                                    content,
                                    error: error_msg,
                                    is_error,
                                };

                                if let Err(e) = manager_ref
                                    .handle_tool_result(
                                        session_id_clone.clone(),
                                        tool_call_id,
                                        result,
                                    )
                                    .await
                                {
                                    log::error!(
                                        "Failed to handle builtin tool result for session {}: {}",
                                        session_id_clone,
                                        e
                                    );
                                }

                                // Emit ToolExecutionCompleted event (success case)
                                let event =
                                    crate::agent::events::AgentEvent::ToolExecutionCompleted {
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
                                    "Builtin tool execution failed for session {}: {}",
                                    session_id_clone,
                                    e
                                );

                                let result = crate::commands::agent_commands::ToolExecutionResult {
                                    success: false,
                                    content: String::new(),
                                    error: Some(e.clone()),
                                    is_error: true,
                                };

                                if let Err(err) = manager_ref
                                    .handle_tool_result(
                                        session_id_clone.clone(),
                                        tool_call_id,
                                        result,
                                    )
                                    .await
                                {
                                    log::error!(
                                        "Failed to handle builtin tool error for session {}: {}",
                                        session_id_clone,
                                        err
                                    );
                                }

                                // Emit ToolExecutionCompleted event (error case)
                                let event =
                                    crate::agent::events::AgentEvent::ToolExecutionCompleted {
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
                } else {
                    // External tool (stdio MCP), emit to frontend for handling
                    let request = ToolExecutionRequest {
                        session_id: session_id.clone(),
                        tool_call: tool_call.clone(),
                    };

                    self.app_handle
                        .emit("tool:execute-request", request)
                        .map_err(|e| format!("Failed to emit tool execution request: {}", e))?;

                    log::debug!(
                        "Emitted external tool execution request: {} for session: {}",
                        tool_call.function.name,
                        session_id
                    );
                }
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
                        Self::create_error_tool_result(
                            &session_id,
                            &tool_call_id,
                            result.error.as_deref().unwrap_or("Unknown error"),
                        )
                    } else {
                        Self::create_tool_result_message(
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

            // 1. Save all tool messages to DB
            let message_repo = crate::state::get_message_repository();
            for msg in accumulated_messages {
                message_repo
                    .insert(&msg)
                    .await
                    .map_err(|e| format!("Failed to save tool message: {}", e))?;
            }

            // 2. Recursively call LLM completion
            // The LLM will see the assistant's tool calls and the subsequent tool results
            self.request_llm_completion(session_id).await?;
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
    ///
    /// Combines:
    /// - Agent base prompt (from agent_config.system_prompt)
    /// - Built-in service contexts (Planning, Knowledge, ContentStore, Workspace)
    /// - (Future) Extension prompts
    async fn build_system_prompt(&self, session_id: &str) -> Result<String, String> {
        let mut parts = Vec::new();

        // 1. Load Agent base prompt
        let active = self.active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        if let Some(agent_config) = session.metadata.agent_config.as_ref() {
            let config = crate::agent::AgentConfig::from_json(agent_config)?;
            if !config.system_prompt.trim().is_empty() {
                parts.push(config.system_prompt);
            }
        }

        drop(active); // Release read lock before getting service contexts

        // 2. Get Built-in service contexts (best-effort)
        if let Some(proxy) = self.proxy_manager.get_proxy(session_id).await {
            let contexts = proxy.get_service_contexts().await;

            if !contexts.is_empty() {
                parts.push("\n\n## Available Tools & Current State\n".to_string());

                for (_tool_id, context_prompt) in contexts {
                    if !context_prompt.trim().is_empty() {
                        parts.push(context_prompt);
                    }
                }
            }
        }

        Ok(parts.join("\n"))
    }

    /// Request LLM completion from frontend
    async fn request_llm_completion(&self, session_id: String) -> Result<(), String> {
        use tauri::Emitter;

        // Get all messages in the session for context
        let message_repo = crate::state::get_message_repository();
        // Get first page with large page size to get all messages
        let page = message_repo
            .get_page(&session_id, 1, 1000)
            .await
            .map_err(|e| format!("Failed to get session messages: {}", e))?;

        let messages = page.items;

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

        let model = agent_config.model;
        let provider = agent_config.provider;
        let temperature = Some(agent_config.temperature);
        let max_tokens = agent_config.max_tokens;

        drop(active); // Release read lock before emitting event

        // Build complete system prompt (agent base + service contexts)
        let system_prompt = Some(self.build_system_prompt(&session_id).await?);

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
        }

        let request = CompletionRequest {
            session_id: session_id.clone(),
            messages,
            model,
            provider,
            system_prompt,
            temperature,
            max_tokens,
        };

        self.app_handle
            .emit("llm:completion-request", request)
            .map_err(|e| format!("Failed to emit LLM completion request: {}", e))?;

        log::info!("Emitted LLM completion request for session: {}", session_id);

        Ok(())
    }

    /// Create a tool result message from successful tool execution
    fn create_tool_result_message(
        session_id: &str,
        tool_call_id: &str,
        content: String,
    ) -> Message {
        let now = chrono::Utc::now().timestamp_millis();

        Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: "tool".to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            content,
            tool_calls: None,
            is_streaming: Some(false),
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            created_at: now,
            updated_at: now,
            source: Some("tool".to_string()),
            error: None,
        }
    }

    /// Create an error tool result message from failed tool execution
    fn create_error_tool_result(
        session_id: &str,
        tool_call_id: &str,
        error_message: &str,
    ) -> Message {
        let now = chrono::Utc::now().timestamp_millis();

        Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: "tool".to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            content: format!("Error: {}", error_message),
            tool_calls: None,
            is_streaming: Some(false),
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            created_at: now,
            updated_at: now,
            source: Some("tool".to_string()),
            error: None,
        }
    }
}

/// Extract builtin tool IDs from agent configuration
///
/// This function analyzes the agent config to determine which builtin MCP tools
/// should be available for the session. Currently, it returns a default set of
/// builtin tools that all agents can use.
///
/// # Arguments
/// * `agent_config` - The agent configuration
///
/// # Returns
/// A vector of builtin tool IDs to initialize for the session
fn extract_builtin_tool_ids(agent_config: &crate::agent::AgentConfig) -> Vec<String> {
    let mut tool_ids = Vec::new();

    // Bootstrap server is always available (platform detection, installation guides)
    tool_ids.push("bootstrap".to_string());

    // Check if specific builtin services are allowed
    // None = all allowed, Some([]) = none allowed, Some([...]) = specific ones allowed
    if let Some(allowed_aliases) = &agent_config.allowed_built_in_service_aliases {
        // If empty list, no builtin services enabled
        if allowed_aliases.is_empty() {
            return tool_ids; // Only bootstrap
        }

        // Check for specific services
        // Note: Knowledge, Planning, Playbook, Assistant will be added in Phase 2/3
        // For now, we only have bootstrap implemented
        for alias in allowed_aliases {
            match alias.as_str() {
                "bootstrap" => {
                    // Already added above
                }
                // Phase 2
                "knowledge" => tool_ids.push("knowledge".to_string()),
                "planning" => tool_ids.push("planning".to_string()),
                // Phase 3
                "playbook" => tool_ids.push("playbook".to_string()),
                "assistant" => tool_ids.push("assistant".to_string()),
                // Essential servers (always enabled if requested)
                "workspace" => tool_ids.push("workspace".to_string()),
                "content_store" | "contentstore" => tool_ids.push("content_store".to_string()),
                "ui" => tool_ids.push("ui".to_string()),
                _ => {
                    log::warn!("Unknown builtin service alias: {}", alias);
                }
            }
        }
    } else {
        // None = all builtin services allowed
        // Phase 2
        tool_ids.push("knowledge".to_string());
        tool_ids.push("planning".to_string());
        // Phase 3
        tool_ids.push("playbook".to_string());
        tool_ids.push("assistant".to_string());
        // Essential servers
        tool_ids.push("workspace".to_string());
        tool_ids.push("content_store".to_string());
        tool_ids.push("ui".to_string());
    }

    tool_ids
}
