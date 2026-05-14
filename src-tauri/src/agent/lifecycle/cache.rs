use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// Load messages from DB into in-memory cache (called once per session)
pub async fn init_session_with_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(), String> {
    let message_repo = crate::state::get_message_repository();

    // Load last N messages from DB (one-time operation).
    // get_page is 1-based: page=1 maps to offset=0 (first page).
    let page = message_repo
        .get_page(session_id, 1, MAX_CACHED_MESSAGES as u64)
        .await
        .map_err(|e| format!("Failed to load messages for session {}: {}", session_id, e))?;

    let loaded_count = page.items.len();

    // Load compact context if exists (SP17)
    let compact_context =
        crate::agent::lifecycle::load_compact_context_record(session_id, "cache init").await?;

    // Populate in-memory cache
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut messages = session.messages.write().await;
        *messages = page.items.clone(); // Clone so we can use page.items later

        let mut synced_at = session.last_synced_at.write().await;
        *synced_at = Some(SystemTime::now());

        crate::agent::lifecycle::replace_compact_context(session, compact_context).await;

        session.cache_initialized.store(true, Ordering::Release);

        // Detect incomplete user turn: last non-recovery message is `user` with no assistant response.
        // This happens after a crash.
        let has_incomplete_turn = page
            .items
            .iter()
            .rfind(|m| !m.is_recovery_message())
            .map(|m| m.role == "user")
            .unwrap_or(false);

        log::info!(
            "Initialized session cache: session={}, messages_loaded={}, incomplete_turn={}",
            session_id,
            loaded_count,
            has_incomplete_turn
        );
    } else {
        return Err(format!("Session not found: {}", session_id));
    }

    Ok(())
}

/// Ensure cache is initialized before workflow starts (lazy initialization)
pub async fn ensure_cache_initialized(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(), String> {
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        if !session.cache_initialized.load(Ordering::Acquire) {
            drop(sessions); // Release read lock before calling init
            init_session_with_messages(active_sessions, session_id).await?;
        }
    }
    Ok(())
}
