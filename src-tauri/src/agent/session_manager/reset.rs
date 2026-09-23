use super::AgentSessionManager;
use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::planning_repository::PlanningRepository;
use tauri::Manager;

/// `/clear`: return quickly with Idle + empty history/pending queue.
///
/// Observed failure mode (session x9b7rtc… 08:12):
/// 1. Clear awaited browser dispose (often 5s+ timeout) BEFORE wiping
///    messages / emitting ResourceUpdated.
/// 2. FE kept showing history and `agent_execute_command` stayed pending,
///    so the user issued another `/clear` (nested clear).
/// 3. Meanwhile a new inject could reach Busy; the late clear then wiped
///    expected_response without re-settling Idle → zombie Busy → later
///    submits sat in pending forever.
///
/// Fix: settle transcript/status first, emit clear, return; browser
/// teardown is best-effort and must not gate the command.
pub(crate) async fn reset_session(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Result<(), String> {
    async fn settle_idle(
        manager: &AgentSessionManager,
        session_id: &str,
        is_active: bool,
    ) -> Result<(), String> {
        if is_active {
            crate::agent::lifecycle::update_session_status(
                &manager.session_repo,
                &manager.active_sessions,
                &manager.app_handle,
                session_id,
                crate::repositories::SessionStatus::Idle,
            )
            .await
        } else {
            manager
                .session_repo
                .update_status(session_id, crate::repositories::SessionStatus::Idle)
                .await
                .map_err(|e| format!("Failed to update session status in DB: {}", e))
        }
    }

    // 0. Cancel workflow if running
    {
        let sessions = manager.active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            session.cancellation_token.cancel();
        }
    }

    // 0b. Transition to Idle early (release active permit / UI feedback)
    let is_active = {
        let sessions = manager.active_sessions.read().await;
        sessions.contains_key(session_id)
    };
    settle_idle(manager, session_id, is_active).await?;

    // 1. Delete messages from DB
    let repo = crate::state::get_message_repository();
    repo.delete_by_session(session_id)
        .await
        .map_err(|e| format!("Failed to delete messages from DB: {}", e))?;

    // 2. Clear planning data (goal, todo, scratchpad)
    let planning_repo = crate::state::get_planning_repository();
    planning_repo
        .clear_session(session_id)
        .await
        .map_err(|e| format!("Failed to clear planning data during reset: {}", e))?;

    // 3. Delete compact context
    let compact_repo = crate::state::get_compact_context_repository();
    if let Err(e) = compact_repo.delete_by_session_id(session_id).await {
        log::warn!("Failed to clear compact context during reset: {}", e);
    }

    // 4. Clear in-memory active session cache
    {
        let mut sessions = manager.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.clear().await;
        }
    }

    // 5. Drop durable + in-memory pending waiters (clear is a hard reset)
    if let Err(e) = crate::agent::pending_queue::discard_all_pending_messages(
        &manager.active_sessions,
        Some(&manager.app_handle),
        session_id,
    )
    .await
    {
        log::warn!(
            "Failed to discard pending messages during reset for {}: {}",
            session_id,
            e
        );
    }

    // 6. Re-cancel + force Idle again — closes mid-clear Busy race window
    {
        let sessions = manager.active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            session.cancellation_token.cancel();
        }
    }
    let is_active = {
        let sessions = manager.active_sessions.read().await;
        sessions.contains_key(session_id)
    };
    settle_idle(manager, session_id, is_active).await?;

    // 7. Notify frontend that history is gone (drives UI clear)
    crate::agent::tauri_events::emit_resource_updated(
        "session",
        "clear",
        Some(session_id.to_string()),
    );

    // 8. Browser dispose can hang for seconds — never block `/clear` on it
    let app_handle = manager.app_handle.clone();
    let browser_session_id = session_id.to_string();
    tauri::async_runtime::spawn(async move {
        let browser_server = app_handle.try_state::<crate::services::InteractiveBrowserServer>();
        if let Some(browser_svc) = browser_server {
            if let Err(e) = browser_svc
                .inner()
                .close_agent_browser_sessions(&browser_session_id)
                .await
            {
                log::warn!("Failed to close browser session during reset: {}", e);
            }
        }
    });

    Ok(())
}
