use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Delete an agent session and all its data
///
/// **Cascade Philosophy:** "When a parent is deleted, its children are also deleted"
/// - DB-level CASCADE automatically deletes child session records
/// - We must manually delete workspace directories for all descendants before DB deletion
pub async fn delete_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<Vec<String>, String> {
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
    let _ = crate::agent::workflow::terminate_session(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id.clone(),
    )
    .await;

    for descendant_id in &descendant_ids {
        let _ = crate::agent::workflow::terminate_session(
            session_repo,
            active_sessions,
            proxy_manager,
            app_handle,
            descendant_id.clone(),
        )
        .await;
    }

    // 2. Remove from active sessions (parent + any loaded descendants)
    {
        let mut sessions = active_sessions.write().await;
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

    let mut deleted_ids = vec![session_id];
    deleted_ids.extend(descendant_ids);

    Ok(deleted_ids)
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
) -> Result<(String, Vec<String>), String> {
    // 1. Terminate workflow if running (this session only)
    let _ = crate::agent::workflow::terminate_session(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id.clone(),
    )
    .await;

    // 2. Remove from active sessions map
    active_sessions.write().await.remove(&session_id);

    // 3. Delete workspace and db
    let orphaned_ids = crate::services::SessionCleanupService::delete_session_data_only(&session_id).await?;

    log::info!(
        "✅ Deleted session only (children orphaned): {}",
        session_id
    );
    Ok((session_id, orphaned_ids))
}
