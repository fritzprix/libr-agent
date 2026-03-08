use crate::agent::state::{AgentSession, MAX_CACHED_MESSAGES};
use crate::repositories::compact_context_repository::CompactContextRepository;
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

    // Load last N messages from DB (one-time operation)
    let page = message_repo
        .get_page(session_id, 0, MAX_CACHED_MESSAGES as u64)
        .await
        .map_err(|e| format!("Failed to load messages for session {}: {}", session_id, e))?;

    let loaded_count = page.items.len();

    // Load compact context if exists (SP17)
    let compact_repo = crate::state::get_compact_context_repository();
    let compact_context = compact_repo
        .get_by_session_id(session_id)
        .await
        .map_err(|e| {
            format!(
                "Failed to load compact context for session {}: {}",
                session_id, e
            )
        })?;

    // Populate in-memory cache
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut messages = session.messages.write().await;
        *messages = page.items; // Replace with DB data

        let mut synced_at = session.last_synced_at.write().await;
        *synced_at = Some(SystemTime::now());

        let mut session_compact = session.compact_context.write().await;
        *session_compact = compact_context;

        session.cache_initialized.store(true, Ordering::Release);

        log::info!(
            "Initialized session cache: session={}, messages_loaded={}",
            session_id,
            loaded_count
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
