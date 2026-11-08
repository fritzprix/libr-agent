use crate::agent::events::AgentEventDispatcher;
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::TauriEventDispatcher;
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
    let dispatcher = TauriEventDispatcher::new(app_handle.clone());
    delete_session_with_dispatcher(
        session_repo,
        active_sessions,
        proxy_manager,
        &dispatcher,
        session_id,
    )
    .await
}

/// Like [`delete_session`], with events sent through `dispatcher`.
pub async fn delete_session_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    dispatcher: &dyn AgentEventDispatcher,
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
    let _ = crate::agent::workflow::terminate_session_with_dispatcher(
        session_repo,
        active_sessions,
        proxy_manager,
        dispatcher,
        session_id.clone(),
    )
    .await;

    for descendant_id in &descendant_ids {
        let _ = crate::agent::workflow::terminate_session_with_dispatcher(
            session_repo,
            active_sessions,
            proxy_manager,
            dispatcher,
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

    let mut deleted_ids = vec![session_id.clone()];
    deleted_ids.extend(descendant_ids);

    // 4. Keep the teardown fences of the deleted sessions (issue #2633 G2.1).
    // A dispatch already in flight when delete started cannot be stopped by
    // the deleted rows alone, so the fence must stay set: clearing it here
    // would let that dispatch lazily recreate builtin instances for a
    // session that no longer exists. The entries are tiny and bounded by the
    // number of sessions in a local app.

    crate::agent::tauri_events::emit_resource_updated("session", "delete", Some(session_id));

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
    let dispatcher = TauriEventDispatcher::new(app_handle.clone());
    delete_session_only_with_dispatcher(
        session_repo,
        active_sessions,
        proxy_manager,
        &dispatcher,
        session_id,
    )
    .await
}

/// Like [`delete_session_only`], with events sent through `dispatcher`.
pub async fn delete_session_only_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
) -> Result<(String, Vec<String>), String> {
    // 1. Terminate workflow if running (this session only)
    let _ = crate::agent::workflow::terminate_session_with_dispatcher(
        session_repo,
        active_sessions,
        proxy_manager,
        dispatcher,
        session_id.clone(),
    )
    .await;

    // 2. Remove from active sessions map
    active_sessions.write().await.remove(&session_id);

    // 3. Delete workspace and db
    let orphaned_ids =
        crate::services::SessionCleanupService::delete_session_data_only(&session_id).await?;

    // Keep the teardown fence set (issue #2633 G2.1). A dispatch that
    // started after the clear would lazily recreate builtin instances for
    // a session whose rows are already gone; only an explicit lifecycle
    // entry may lift the fence, and that can never happen for a deleted id.

    log::info!(
        "✅ Deleted session only (children orphaned): {}",
        session_id
    );

    crate::agent::tauri_events::emit_resource_updated(
        "session",
        "delete",
        Some(session_id.clone()),
    );

    Ok((session_id, orphaned_ids))
}
