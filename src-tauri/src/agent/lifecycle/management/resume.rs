use super::status::record_session_error_with_dispatcher;
use crate::agent::context::registry::ContextRegistry;
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{SessionMetadata, SessionStatus};
use std::collections::HashMap;
use std::path::PathBuf;
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
    _context_registry: Arc<ContextRegistry>,
    session_id: &str,
) -> Result<SessionMetadata, String> {
    // Get session metadata from database using injected repository
    let session = session_repo
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
            // Keep the preference — transient path unavailability must not wipe it.
            log::warn!(
                "Persisted workspace override '{}' for session {} is unavailable or not a directory; \
                 keeping the preference and falling back to the default workspace until it returns.",
                workspace_override,
                session_id
            );
        }
    }

    if session.workspace_isolation
        == crate::models::workspace_isolation::WorkspaceIsolationMode::Docker
    {
        if session.status == SessionStatus::Provisioning {
            crate::services::docker_provisioning::ensure_provisioning_task_for_session(
                &crate::services::docker_provisioning::DockerProvisioningDeps {
                    session_repo: Arc::clone(session_repo),
                    active_sessions: Arc::clone(active_sessions),
                    proxy_manager: Arc::clone(proxy_manager),
                    app_handle: app_handle.clone(),
                },
                &session,
            )
            .await?;
        } else if let Err(error) =
            crate::services::WorkspaceRuntimeManager::ensure_runtime_cached(&session).await
        {
            // Store this failure: an older reason may name a different cause.
            let reason =
                crate::services::docker_provisioning::failure_reason(&session, &error.to_string());
            let dispatcher = TauriEventDispatcher::new(app_handle.clone());
            if let Err(record_error) = record_session_error_with_dispatcher(
                session_repo,
                active_sessions,
                &dispatcher,
                session_id,
                &reason,
            )
            .await
            {
                log::warn!(
                    "Failed to record Docker runtime failure for session {}: {}",
                    session_id,
                    record_error
                );
            }
            return Err(reason);
        }
    }

    // Deserialize agent config (live assistant settings + session lineage)
    // SSOT: MCP activation is session → assistant → mcpServerIds.
    let (tool_ids, mcp_server_ids) = crate::agent::resolve_session_mcp_bindings(&session).await?;

    // Load compact context if exists (SP17)
    let compact_context_record =
        crate::agent::lifecycle::load_compact_context_record(session_id, "session resume").await?;

    // Activate session in memory first so open/resume can return without waiting on MCP.
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(session_id) {
        log::info!(
            "Session {} already active in memory, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
        // Invalidate cached stable prompt — metadata (assistant binding, name) may have
        // changed between sessions, so it must be rebuilt on the next LLM call.
        *existing_session.cached_stable_prompt.write().await = None;
        // Update compact context if it was loaded
        crate::agent::lifecycle::set_compact_context_if_loaded(
            existing_session,
            compact_context_record,
        )
        .await;
        // Transient states (pending_execution, messages, cancellation_token, etc.) are preserved
    } else {
        log::info!(
            "Session {} not in memory, initializing new active session state",
            session_id
        );
        // Resume historically initializes an empty registry (caller arg unused).
        active.insert(
            session_id.to_string(),
            AgentSession::new(
                session.clone(),
                Arc::new(ContextRegistry::new()),
                compact_context_record,
            ),
        );
    }
    drop(active);

    // MCP proxy creation (incl. HTTP startup) must not block session UI.
    // Workflow soft-waits via ensure_proxy_ready; discovery surfaces via runtime-state events.
    crate::agent::lifecycle::spawn_create_proxy_for_session(
        Arc::clone(proxy_manager),
        app_handle.clone(),
        session_id.to_string(),
        tool_ids,
        mcp_server_ids,
    )
    .await;

    log::info!("Resumed agent session: {}", session_id);
    Ok(session)
}
