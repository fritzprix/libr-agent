use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Resume an existing session by loading it into active sessions
#[allow(dead_code)]
pub async fn resume_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    context_registry: Arc<crate::agent::context::registry::ContextRegistry>,
    session_id: &str,
) -> Result<crate::repositories::SessionMetadata, String> {
    // Get session metadata from database using injected repository
    let session = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

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

    // Add to active sessions with cancellation token and empty cache
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(session_id) {
        log::info!(
            "Session {} already active in memory, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
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
                cancellation_token: tokio_util::sync::CancellationToken::new(),
                cancel_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                pending_execution: None,
                messages: Arc::new(RwLock::new(Vec::new())),
                cache_initialized: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                last_synced_at: Arc::new(RwLock::new(None)),
                thinking_only_count: Arc::new(RwLock::new(0)),
                pending_events: Arc::new(RwLock::new(
                    crate::agent::state::PendingEventManager::new(),
                )),
                context_registry,
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
    }

    log::info!("Updated agent config for session: {}", session_id);
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
    // SP2: Acquire/release active-agent slot based on the status transition direction.
    let prev_status = {
        let active = active_sessions.read().await;
        active.get(session_id).map(|s| s.metadata.status.clone())
    };
    let is_prev_busy = matches!(prev_status, Some(SessionStatus::Busy));
    let is_next_busy = status == SessionStatus::Busy;

    let gate = crate::state::get_concurrency_gate();
    match (is_prev_busy, is_next_busy) {
        (false, true) => {
            // Entering Busy: acquire an active-agent slot (blocks until one is free).
            gate.acquire_active_agent().await?;
        }
        (true, false) => {
            // Leaving Busy: release the slot so another session can run.
            gate.release_active_agent();
        }
        _ => {} // re-entrancy (Busy→Busy) or non-Busy→non-Busy: no-op
    }

    session_repo
        .update_status(session_id, status.clone())
        .await
        .map_err(|e| format!("Failed to update session status: {}", e))?;

    // Update in-memory state
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(session_id) {
        session.metadata.status = status.clone();
    }
    drop(active); // Release write lock before waking waiters.

    // SP1: Wake any tasks sleeping on this session's status change (e.g. awaitAgent).
    crate::state::get_session_bus().notify_status_change(session_id);

    // Emit status changed event
    let event = crate::agent::events::AgentEvent::StatusChanged {
        session_id: session_id.to_string(),
        status,
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(())
}

/// Recursively collect all descendant session IDs (children, grandchildren, etc.)
pub(crate) async fn collect_descendant_ids(session_id: &str) -> Result<Vec<String>, String> {
    use crate::repositories::session_repository::SessionRepository as SessionRepositoryTrait;

    let session_repo = crate::state::get_session_repository();
    let mut all_descendants = Vec::new();
    let mut queue = vec![session_id.to_string()];

    while let Some(current_id) = queue.pop() {
        let children = session_repo
            .get_child_session_ids(&current_id)
            .await
            .map_err(|e| format!("Failed to get children for {}: {}", current_id, e))?;

        for child_id in children {
            all_descendants.push(child_id.clone());
            queue.push(child_id);
        }
    }

    Ok(all_descendants)
}

/// Delete workspace directory for a session
pub(crate) async fn delete_session_workspace(session_id: &str) -> Result<(), String> {
    match crate::session::get_session_manager() {
        Ok(manager) => {
            // Ensure workspace is loaded into pool before attempting removal
            let _ = manager.get_session_workspace_dir_by_id(session_id);
            if let Err(e) = manager.remove_session(session_id).await {
                log::warn!(
                    "Failed to remove workspace for session {}: {}",
                    session_id,
                    e
                );
            }
        }
        Err(e) => {
            log::warn!("Failed to get session manager for workspace cleanup: {}", e);
        }
    }
    Ok(())
}

/// Delete an agent session and all its data
///
/// **Cascade Philosophy:** "부모를 지우면 자식도 지워진다"
/// - DB-level CASCADE automatically deletes child session records
/// - We must manually delete workspace directories for all descendants before DB deletion
pub async fn delete_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    use crate::repositories::session_repository::SessionRepository as SessionRepositoryTrait;

    // 0. Collect all descendant IDs BEFORE cascade delete (so we can clean their workspaces)
    log::debug!(
        "Collecting descendants for cascade workspace cleanup: {}",
        session_id
    );
    let descendant_ids = collect_descendant_ids(&session_id).await?;

    if !descendant_ids.is_empty() {
        log::info!(
            "🌲 Cascade delete: {} will remove {} descendant session(s)",
            session_id,
            descendant_ids.len()
        );
    }

    // 1. Terminate workflow if running (for this session and all descendants)
    let _ = crate::agent::workflow::terminate_session(session_repo, active_sessions, proxy_manager, app_handle, session_id.clone()).await;
    for descendant_id in &descendant_ids {
        let _ = crate::agent::workflow::terminate_session(session_repo, active_sessions, proxy_manager, app_handle, descendant_id.clone()).await;
    }

    // 2. Remove from active sessions (parent only - descendants might not be in memory)
    active_sessions.write().await.remove(&session_id);

    // 3. Delete workspaces for all descendants BEFORE DB cascade
    //    (DB CASCADE will delete records, but not filesystem directories)
    for descendant_id in &descendant_ids {
        delete_session_workspace(descendant_id).await?;

        // Also delete search index (filesystem)
        if let Err(e) = crate::search::index_storage::delete_index(descendant_id) {
            log::warn!(
                "Failed to delete search index for descendant {}: {}",
                descendant_id,
                e
            );
        }
    }

    // 4. Delete workspace and search index for the parent session
    delete_session_workspace(&session_id).await?;

    if let Err(e) = crate::search::index_storage::delete_index(&session_id) {
        log::warn!(
            "Failed to delete search index for session {}: {}",
            session_id,
            e
        );
    }

    // 5. Delete from database (CASCADE will automatically delete all descendant records)
    //    - Child sessions (via FK parent_session_id)
    //    - All messages (via FK session_id)
    //    - Index metadata (via FK session_id, if exists)
    let session_repo = crate::state::get_session_repository();
    session_repo
        .delete_session(&session_id)
        .await
        .map_err(|e| format!("Failed to delete session metadata: {}", e))?;

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
pub async fn delete_session_only(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    use crate::repositories::session_repository::SessionRepository as SessionRepositoryTrait;

    // 1. Terminate workflow if running (this session only)
    let _ = crate::agent::workflow::terminate_session(session_repo, active_sessions, proxy_manager, app_handle, session_id.clone()).await;

    // 2. Remove from active sessions map
    active_sessions.write().await.remove(&session_id);

    // 3. Delete workspace and search index for this session only
    delete_session_workspace(&session_id).await?;

    if let Err(e) = crate::search::index_storage::delete_index(&session_id) {
        log::warn!(
            "Failed to delete search index for session {}: {}",
            session_id,
            e
        );
    }

    // 4. Orphan direct children and delete from DB
    let session_repo = crate::state::get_session_repository();
    session_repo
        .orphan_and_delete_session(&session_id)
        .await
        .map_err(|e| format!("Failed to delete session metadata: {}", e))?;

    log::info!(
        "✅ Deleted session only (children orphaned): {}",
        session_id
    );
    Ok(())
}
