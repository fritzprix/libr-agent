use crate::agent::state::AgentSession;
use crate::repositories::session_repository::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Update model/provider (and optionally assistant binding) for an existing session
#[allow(clippy::too_many_arguments)]
pub async fn update_session_config(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    _app_handle: &AppHandle,
    session_id: &str,
    model: Option<String>,
    provider: Option<String>,
    assistant_id: Option<String>,
    recursive: Option<bool>,
) -> Result<(), String> {
    let mut session_ids = vec![session_id.to_string()];

    if recursive.unwrap_or(false) {
        let descendant_ids =
            crate::services::SessionCleanupService::collect_descendant_ids(session_id).await?;
        session_ids.extend(descendant_ids);
    }

    for id in &session_ids {
        session_repo
            .update_session_config(id, model.clone(), provider.clone())
            .await
            .map_err(|e| format!("Failed to update session config for {}: {}", id, e))?;

        if id == session_id {
            if let Some(ref assistant_id) = assistant_id {
                session_repo
                    .update_assistant_id(id, Some(assistant_id.clone()))
                    .await
                    .map_err(|e| {
                        format!("Failed to update session assistant_id for {}: {}", id, e)
                    })?;
            }
        }
    }

    let mut active = active_sessions.write().await;
    let now = chrono::Utc::now().timestamp_millis();
    for id in &session_ids {
        if let Some(session) = active.get_mut(id) {
            if let Some(ref m) = model {
                session.metadata.model = m.clone();
            }
            if let Some(ref p) = provider {
                session.metadata.provider = p.clone();
            }
            if id == session_id {
                if let Some(ref id) = assistant_id {
                    session.metadata.assistant_id = Some(id.clone());
                }
            }
            session.metadata.updated_at = now;
            *session.cached_stable_prompt.write().await = None;
        }
    }
    drop(active);

    // Emit resource update event to trigger frontend session list refresh
    crate::agent::tauri_events::emit_resource_updated(
        "session",
        "update",
        Some(session_id.to_string()),
    );

    log::info!(
        "Updated agent config for session: {} (recursive: {:?})",
        session_id,
        recursive
    );

    Ok(())
}
