use super::AgentSessionManager;

/// Reload session tools, skills, and instructions context without wiping conversation history.
///
/// Recreates the session MCP proxy to refresh tool schemas and assistant bindings,
/// invalidates the in-memory prompt cache, and emits a session update event for frontend
/// skills and tools revalidation.
pub(crate) async fn reload_session(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Result<(), String> {
    use crate::repositories::SessionStatus;

    // 1. Status guard: reject busy, queued, provisioning, or in-flight transitions/compaction
    let (status, is_transitioning, compaction_in_flight) = {
        let active = manager.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let trans = session.status_transition.read().await;
            let transitioning = matches!(
                trans.as_ref(),
                Some(crate::agent::state::SessionStatusTransition::ToStatus(
                    SessionStatus::Busy | SessionStatus::Queued
                ))
            );
            let compacting = session.compaction.snapshot().await.is_in_flight();
            (session.metadata.status.clone(), transitioning, compacting)
        } else {
            let meta = manager
                .session_repo
                .get_session(session_id)
                .await
                .map_err(|e| format!("Failed to load session {}: {}", session_id, e))?
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            (meta.status, false, false)
        }
    };

    validate_session_reloadable(&status, is_transitioning, compaction_in_flight)?;

    // 2. Invalidate active session stable prompt cache so the next LLM turn rebuilds it
    {
        let active = manager.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            *session.cached_stable_prompt.write().await = None;
        }
    }

    // 3. Force MCP proxy recreate to pick up changed bindings or tool schemas
    manager.proxy_manager.destroy_proxy(session_id).await;
    manager
        .proxy_manager
        .ensure_configured_proxy(session_id, Some(manager.app_handle.clone()))
        .await?;

    // 4. Invalidate skills scan cache so disk changes are picked up on revalidation
    crate::services::skill_service::invalidate_skill_scan_cache();

    // 5. Emit session resource update to trigger skills / tool discovery revalidation on frontend
    crate::agent::tauri_events::emit_resource_updated(
        "session",
        "update",
        Some(session_id.to_string()),
    );

    Ok(())
}

pub(crate) fn validate_session_reloadable(
    status: &crate::repositories::SessionStatus,
    is_transitioning: bool,
    is_compacting: bool,
) -> Result<(), String> {
    use crate::repositories::SessionStatus;

    if is_transitioning || is_compacting {
        return Err(
            "Cannot reload session while an execution or compaction is in progress.".to_string(),
        );
    }

    match status {
        SessionStatus::Busy | SessionStatus::Queued | SessionStatus::Provisioning => Err(format!(
            "Cannot reload session while it is {}.",
            status.as_str()
        )),
        SessionStatus::Idle | SessionStatus::Paused | SessionStatus::Error => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_session_reloadable;
    use crate::repositories::SessionStatus;

    #[test]
    fn test_validate_session_reloadable_status_guards() {
        assert!(validate_session_reloadable(&SessionStatus::Idle, false, false).is_ok());
        assert!(validate_session_reloadable(&SessionStatus::Paused, false, false).is_ok());
        assert!(validate_session_reloadable(&SessionStatus::Error, false, false).is_ok());

        assert!(validate_session_reloadable(&SessionStatus::Busy, false, false).is_err());
        assert!(validate_session_reloadable(&SessionStatus::Queued, false, false).is_err());
        assert!(validate_session_reloadable(&SessionStatus::Provisioning, false, false).is_err());

        assert!(validate_session_reloadable(&SessionStatus::Idle, true, false).is_err());
        assert!(validate_session_reloadable(&SessionStatus::Idle, false, true).is_err());
    }
}
