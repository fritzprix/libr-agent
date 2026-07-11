use crate::repositories::session_repository::SessionRepository;
use crate::session::SessionManager;
use std::path::{Path, PathBuf};

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
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(session_id);
    let _ = ensure_teamwork_link(session_repo, session_manager, session_id, &workspace_dir).await;
    Ok(workspace_dir)
}

async fn resolve_teamwork_root_session_id(
    session_repo: &dyn SessionRepository,
    session_id: &str,
) -> Result<String, String> {
    let mut current = match session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to load session metadata: {e}"))?
    {
        Some(session) => session,
        None => return Ok(session_id.to_string()),
    };

    if let Some(org_root_session_id) = current.org_root_session_id.clone() {
        return Ok(org_root_session_id);
    }

    // Traverse parent chain limit
    for _ in 0..64 {
        let Some(parent_session_id) = current.parent_session_id.clone() else {
            return Ok(current.id);
        };

        current = match session_repo
            .get_session(&parent_session_id)
            .await
            .map_err(|e| format!("Failed to load parent session metadata: {e}"))?
        {
            Some(session) => session,
            None => return Ok(parent_session_id),
        };

        if let Some(org_root_session_id) = current.org_root_session_id.clone() {
            return Ok(org_root_session_id);
        }
    }

    Ok(current.id)
}

async fn ensure_teamwork_link(
    session_repo: &dyn SessionRepository,
    session_manager: &SessionManager,
    session_id: &str,
    workspace_root: &Path,
) -> Result<(), String> {
    let teamwork_root_session_id =
        resolve_teamwork_root_session_id(session_repo, session_id).await?;
    let teamwork_dir = session_manager
        .get_directory_service()
        .get_teamwork_artifact_dir_unverified(&teamwork_root_session_id);

    // If the teamwork directory itself doesn't exist yet, we can't link to it.
    if !teamwork_dir.exists() {
        return Ok(());
    }

    let link_parent = workspace_root.join(".libragent");
    if !link_parent.exists() {
        if let Err(e) = std::fs::create_dir_all(&link_parent) {
            log::warn!("Failed to create .libragent folder: {}", e);
            return Ok(());
        }
    }
    let link_path = link_parent.join("teamwork");

    let is_valid = if link_path.exists() || link_path.is_symlink() {
        match std::fs::read_link(&link_path) {
            Ok(target) => {
                if target == teamwork_dir {
                    true
                } else {
                    log::info!(
                        "Teamwork link points to wrong target: {:?}, expected: {:?}",
                        target,
                        teamwork_dir
                    );
                    false
                }
            }
            Err(_) => false,
        }
    } else {
        false
    };

    if !is_valid {
        // Remove existing file/symlink/directory if present
        if link_path.exists() || link_path.is_symlink() {
            let _ = std::fs::remove_file(&link_path);
            let _ = std::fs::remove_dir_all(&link_path);
        }

        // Create symlink or junction
        log::info!(
            "Creating teamwork link from {:?} to {:?}",
            link_path,
            teamwork_dir
        );
        let link_res = create_symlink_or_junction(&teamwork_dir, &link_path);
        if let Err(e) = link_res {
            log::warn!("Failed to create teamwork symlink/junction: {}", e);
        }
    }

    Ok(())
}

fn create_symlink_or_junction(target: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        // Use junction command on Windows to avoid requiring developer mode/admin rights
        let mut cmd = std::process::Command::new("cmd");
        cmd.arg("/c").arg("mklink").arg("/j").arg(link).arg(target);
        let output = cmd.output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ))
        }
    }
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
