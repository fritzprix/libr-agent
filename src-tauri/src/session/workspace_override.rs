use crate::repositories::session_repository::SessionRepository;
use crate::session::SessionManager;
use std::path::PathBuf;

/// Synchronize a persisted session workspace override from the DB into SessionManager.
///
/// This keeps runtime workspace resolution aligned with the persisted session metadata,
/// even when the session has not been fully resumed yet.
pub async fn hydrate_persisted_workspace_override(
    session_repo: &dyn SessionRepository,
    session_manager: &SessionManager,
    session_id: &str,
) -> Result<Option<PathBuf>, String> {
    let session = session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to load session {}: {}", session_id, e))?;

    let Some(session) = session else {
        return Ok(None);
    };

    let Some(workspace_override) = session.workspace_override else {
        return Ok(None);
    };

    let path = PathBuf::from(&workspace_override);
    if path.is_dir() {
        session_manager
            .register_session_override(session_id, path.clone())
            .await
            .map_err(|e| {
                format!(
                    "Failed to register persisted workspace override for session {}: {}",
                    session_id, e
                )
            })?;
        return Ok(Some(path));
    }

    log::warn!(
        "Persisted workspace override '{}' for session {} no longer exists or is not a directory; \
         clearing it and falling back to default workspace.",
        workspace_override,
        session_id
    );
    let _ = session_repo
        .update_workspace_override(session_id, None)
        .await;
    let _ = session_manager.remove_workspace_override(session_id).await;
    Ok(None)
}

/// Resolve the effective workspace directory for a session after hydrating any
/// persisted DB override into SessionManager.
pub async fn ensure_session_workspace_dir(
    session_repo: &dyn SessionRepository,
    session_manager: &SessionManager,
    session_id: &str,
) -> Result<PathBuf, String> {
    hydrate_persisted_workspace_override(session_repo, session_manager, session_id).await?;
    Ok(session_manager.get_session_workspace_dir_by_id(session_id))
}

/// Resolve the effective workspace directory for a session, hydrating from the
/// global session repository when it is available and otherwise falling back to
/// the current in-memory SessionManager state.
pub async fn resolve_session_workspace_dir(
    session_manager: &SessionManager,
    session_id: &str,
) -> Result<PathBuf, String> {
    if let Some(session_repo) = crate::state::try_get_session_repository() {
        return ensure_session_workspace_dir(session_repo, session_manager, session_id).await;
    }

    Ok(session_manager.get_session_workspace_dir_by_id(session_id))
}

/// Best-effort hydration from the global session repository when it is
/// available. This avoids test-only panics in flows that operate without the
/// full application state initialized.
pub async fn hydrate_persisted_workspace_override_from_global(
    session_manager: &SessionManager,
    session_id: &str,
) -> Result<(), String> {
    if let Some(session_repo) = crate::state::try_get_session_repository() {
        hydrate_persisted_workspace_override(session_repo, session_manager, session_id).await?;
    }

    Ok(())
}
