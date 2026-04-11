use crate::agent::concurrency::ActiveAgentPermit;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::events::{AgentEvent, AgentEventDispatcher, TauriEventDispatcher};
use crate::agent::state::{AgentSession, SessionStatusTransition};
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{CompactContextRepository, SessionMetadata, SessionStatus};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Resume an existing session by loading it into active sessions
#[allow(dead_code)]
pub async fn resume_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    _context_registry: Arc<ContextRegistry>,
    session_id: &str,
) -> Result<SessionMetadata, String> {
    // Get session metadata from database using injected repository
    let mut session = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    if let Some(workspace_override) = session.workspace_override.as_deref() {
        let path = PathBuf::from(workspace_override);
        if path.is_dir() {
            if let Ok(session_manager) = crate::session::get_session_manager() {
                if let Err(e) = session_manager
                    .register_session_override(session_id, path)
                    .await
                {
                    log::warn!(
                        "Failed to pre-register workspace override for resumed session {}: {}",
                        session_id,
                        e
                    );
                }
            }
        } else {
            log::warn!(
                "Persisted workspace override '{}' for session {} no longer exists or is not a directory; \
                 clearing it and falling back to default workspace.",
                workspace_override,
                session_id
            );
            let _ = session_repo
                .update_workspace_override(session_id, None)
                .await;
            session.workspace_override = None;
        }
    }

    // Deserialize agent config
    let agent_config = if let Some(config_json) = &session.agent_config {
        crate::agent::AgentConfig::from_json(config_json)?
    } else {
        return Err(format!("Session {} has no agent config", session_id));
    };

    // Extract builtin tool IDs from agent config
    let tool_ids = crate::agent::tools::extract_builtin_tool_ids(&agent_config);
    let mcp_server_ids = agent_config.mcp_server_ids.clone();

    // Create proxy for this session
    proxy_manager
        .create_proxy(
            session_id.to_string(),
            tool_ids,
            mcp_server_ids,
            Some(app_handle.clone()),
        )
        .await?;

    log::info!(
        "Created MCP proxy for resumed session: {} with builtin tools",
        session_id
    );

    // Load compact context if exists (SP17)
    let compact_context_record = {
        let repo = crate::state::get_compact_context_repository();
        repo.get_by_session_id(session_id)
            .await
            .map_err(|e| format!("Failed to get compact context: {}", e))?
    };

    if let Some(record) = &compact_context_record {
        log::info!(
            "Loaded compact context for resumed session: {} (range: {} to {})",
            session_id,
            record.from_id,
            record.to_id
        );
    }

    // Add to active sessions with cancellation token and empty cache
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(session_id) {
        log::info!(
            "Session {} already active in memory, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
        // Invalidate cached stable prompt — metadata (agent_config, name) may have
        // changed between sessions, so it must be rebuilt on the next LLM call.
        *existing_session.cached_stable_prompt.write().await = None;
        // Update compact context if it was loaded
        if let Some(record) = compact_context_record {
            let mut compact = existing_session.compact_context.write().await;
            *compact = Some(record);
        }
        // Transient states (pending_execution, messages, cancellation_token, etc.) are preserved
    } else {
        log::info!(
            "Session {} not in memory, initializing new active session state",
            session_id
        );
        active.insert(
            session_id.to_string(),
            AgentSession {
                metadata: session.clone(),
                is_running: false,
                active_permit: None,
                status_transition: Arc::new(RwLock::new(None)),
                transition_lock: Arc::new(tokio::sync::Mutex::new(())),
                cancellation_token: CancellationToken::new(),
                yolo_mode: Arc::new(AtomicBool::new(session.yolo_mode)),
                cancel_pending: Arc::new(AtomicBool::new(false)),
                pending_execution: None,
                messages: Arc::new(RwLock::new(Vec::new())),
                cache_initialized: Arc::new(AtomicBool::new(false)),
                last_synced_at: Arc::new(RwLock::new(None)),
                thinking_only_count: Arc::new(RwLock::new(0)),
                pending_events: Arc::new(RwLock::new(
                    crate::agent::state::PendingEventManager::new(),
                )),
                pending_approvals: Arc::new(RwLock::new(std::collections::HashMap::new())),
                context_registry: Arc::new(crate::agent::context::registry::ContextRegistry::new()),
                compact_context: Arc::new(RwLock::new(compact_context_record)),
                compact_in_flight: Arc::new(AtomicBool::new(false)),
                last_compacted_tail_id: Arc::new(RwLock::new(None)),
                awaiting_compact_completion: Arc::new(AtomicBool::new(false)),
                finalize_workflow_after_compact: Arc::new(AtomicBool::new(false)),
                deferred_workflow_step: Arc::new(RwLock::new(None)),
                compact_started_at_ms: Arc::new(RwLock::new(None)),
                expected_response_id: Arc::new(RwLock::new(None)),
                cached_stable_prompt: Arc::new(RwLock::new(None)),
                last_completion_request: Arc::new(RwLock::new(None)),
            },
        );
    }

    log::info!("Resumed agent session: {}", session_id);
    Ok(session)
}

/// Update agent configuration for an existing session
pub async fn update_session_config(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    _app_handle: &AppHandle,
    session_id: &str,
    model: Option<String>,
    provider: Option<String>,
    agent_config: crate::agent::AgentConfig,
) -> Result<(), String> {
    // 1. Validate new config
    agent_config.validate()?;

    // 2. Serialize config
    let config_json = agent_config.to_json()?;

    // 3. Update in database using injected repository
    session_repo
        .update_session_config(
            session_id,
            model.clone(),
            provider.clone(),
            Some(config_json.clone()),
        )
        .await
        .map_err(|e| format!("Failed to update session config: {}", e))?;

    // 4. Update active session in memory
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(session_id) {
        if let Some(m) = model {
            session.metadata.model = m;
        }
        if let Some(p) = provider {
            session.metadata.provider = p;
        }
        session.metadata.agent_config = Some(config_json);
        session.metadata.updated_at = chrono::Utc::now().timestamp_millis();
        // Invalidate the stable prompt cache — the agent_config (system_prompt, etc.)
        // has changed so it must be rebuilt on the next LLM call.
        *session.cached_stable_prompt.write().await = None;
    }

    log::info!("Updated agent config for session: {}", session_id);

    // 5. Emit event to notify frontend of config change
    // We reuse StatusChanged or create a new event.
    // Ideally we should have a `ConfigChanged` event, but `StatusChanged` might force a refresh if the frontend re-fetches.
    // However, to be explicit, let's just log for now. The frontend will update its local state optimistically or re-fetch.
    // Or we can emit `AgentEvent::StatusChanged` if we want to force some side-effects, but that's hacky.

    // Let's rely on the command response for the immediate update,
    // and if we need broadcast, we'd add `ConfigChanged` to `AgentEvent`.
    // For now, simple DB update is enough as the frontend driving this change will know it happened.

    Ok(())
}

/// Update session status in database and emit event
pub async fn update_session_status(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    status: SessionStatus,
) -> Result<(), String> {
    let dispatcher = TauriEventDispatcher::new(app_handle.clone());
    update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        &dispatcher,
        session_id,
        status,
    )
    .await
}

pub async fn update_session_status_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    status: SessionStatus,
) -> Result<(), String> {
    let (initial_status, transition_lock, transition_handle) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.metadata.status.clone(),
            Arc::clone(&session.transition_lock),
            Arc::clone(&session.status_transition),
        )
    };
    let _transition_guard = transition_lock.lock().await;

    let prev_status = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        if session.metadata.status != initial_status {
            session.metadata.status.clone()
        } else {
            initial_status
        }
    };
    let is_prev_busy = prev_status == SessionStatus::Busy;
    let is_next_busy = status == SessionStatus::Busy;
    let mut acquired_permit: Option<ActiveAgentPermit> = None;
    let mut released_permit: Option<ActiveAgentPermit> = None;

    *transition_handle.write().await = Some(SessionStatusTransition::ToStatus(status.clone()));

    if !matches!((is_prev_busy, is_next_busy), (false, false) | (true, true)) {
        let gate = crate::state::get_concurrency_gate();
        match (is_prev_busy, is_next_busy) {
            (false, true) => {
                acquired_permit = Some(gate.acquire_active_agent().await?);
            }
            (true, false) => {
                let mut active = active_sessions.write().await;
                let session = active
                    .get_mut(session_id)
                    .ok_or_else(|| format!("Session not found: {}", session_id))?;
                released_permit = session.active_permit.take();
            }
            _ => {}
        }
    }

    let persist_result = session_repo.update_status(session_id, status.clone()).await;

    if let Err(error) = persist_result {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(session_id) {
            if let Some(permit) = acquired_permit.take() {
                drop(permit);
            }
            if let Some(permit) = released_permit.take() {
                session.active_permit = Some(permit);
            }
        }
        *transition_handle.write().await = None;
        return Err(format!("Failed to update session status: {}", error));
    }

    // Update in-memory state
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(session_id) {
        if let Some(permit) = acquired_permit.take() {
            session.active_permit = Some(permit);
        }
        session.metadata.status = status.clone();
        // SP3: Update is_running based on status.
        // Busy means the workflow is active/running.
        session.is_running = status == SessionStatus::Busy;
    }
    *transition_handle.write().await = None;
    drop(active); // Release write lock before waking waiters.

    // SP1: Wake any tasks sleeping on this session's status change (e.g. awaitAgent).
    crate::state::get_session_bus().notify_status_change(session_id);

    // Emit status changed event
    let event = AgentEvent::StatusChanged {
        session_id: session_id.to_string(),
        status,
    };
    dispatcher.emit_agent_event(event)?;

    Ok(())
}
