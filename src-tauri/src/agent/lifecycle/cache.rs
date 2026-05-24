use crate::agent::state::{
    AgentSession, CacheInitializationClaim, CompactRepairState, MAX_CACHED_MESSAGES,
};
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

/// Load messages from DB into in-memory cache (called once per session)
pub async fn init_session_with_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(), String> {
    let message_repo = crate::state::get_message_repository();

    // Load the most recent N messages from DB so the runtime cache lines up with
    // persisted compaction boundaries and the active workflow tail.
    let recent_slice = message_repo
        .get_recent_slice(session_id, MAX_CACHED_MESSAGES as u64)
        .await
        .map_err(|e| format!("Failed to load messages for session {}: {}", session_id, e))?;

    let loaded_count = recent_slice.items.len();
    let has_incomplete_turn = recent_slice
        .items
        .iter()
        .rfind(|m| !m.is_recovery_message())
        .map(|m| m.role == "user")
        .unwrap_or(false);

    // Load compact context if exists (SP17)
    let compact_context =
        crate::agent::lifecycle::load_compact_context_record(session_id, "cache init").await?;

    // Populate in-memory cache
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut messages = session.messages.write().await;
        *messages = recent_slice.items;

        let mut synced_at = session.last_synced_at.write().await;
        *synced_at = Some(SystemTime::now());

        let needs_compact_repair = recent_slice.has_more_before
            && compact_context.is_none()
            && session.metadata.status == SessionStatus::Error;

        crate::agent::lifecycle::overwrite_compact_context(session, compact_context).await;

        session.set_compact_repair_state(if needs_compact_repair {
            CompactRepairState::Needed
        } else {
            CompactRepairState::NotNeeded
        });

        session.mark_cache_initialized();

        log::info!(
            "Initialized session cache: session={}, messages_loaded={}, incomplete_turn={}, compact_repair_state={:?}",
            session_id,
            loaded_count,
            has_incomplete_turn,
            session.compact_repair_state()
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
    loop {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            match session.try_claim_cache_initialization() {
                CacheInitializationClaim::Ready => {}
                CacheInitializationClaim::Claimed => {
                    drop(sessions); // Release read lock before calling init
                    if let Err(error) =
                        init_session_with_messages(active_sessions, session_id).await
                    {
                        let sessions = active_sessions.read().await;
                        if let Some(session) = sessions.get(session_id) {
                            session.reset_cache_initialization();
                        }
                        return Err(error);
                    }
                }
                CacheInitializationClaim::InProgress => {
                    drop(sessions);
                    let state = loop {
                        let sessions = active_sessions.read().await;
                        let state = sessions
                            .get(session_id)
                            .map(|session| session.cache_initialization_state());
                        drop(sessions);

                        match state {
                            Some(crate::agent::state::CacheInitializationState::Initializing) => {
                                sleep(Duration::from_millis(10)).await;
                            }
                            other => break other,
                        }
                    };

                    match state {
                        Some(crate::agent::state::CacheInitializationState::Ready) | None => {}
                        Some(crate::agent::state::CacheInitializationState::Uninitialized) => {
                            continue;
                        }
                        Some(crate::agent::state::CacheInitializationState::Initializing) => {
                            unreachable!("initializing state should be handled by the wait loop")
                        }
                    }
                }
            }
        }
        return Ok(());
    }
}
