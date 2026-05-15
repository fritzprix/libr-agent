use crate::agent::state::AgentSession;
use crate::repositories::{
    compact_context_repository::CompactContextRepository, CompactContextRecord,
};

pub async fn load_compact_context_record(
    session_id: &str,
    operation: &str,
) -> Result<Option<CompactContextRecord>, String> {
    let repo = crate::state::get_compact_context_repository();
    let compact_context = repo.get_by_session_id(session_id).await.map_err(|e| {
        format!(
            "Failed to load compact context for session {} during {}: {}",
            session_id, operation, e
        )
    })?;

    if let Some(record) = &compact_context {
        log::info!(
            "Loaded compact context during {}: session={} (range: {} to {})",
            operation,
            session_id,
            record.from_id,
            record.to_id
        );
    }

    Ok(compact_context)
}

/// Overwrite the in-memory compact context, even when the loaded value is `None`.
pub async fn overwrite_compact_context(
    session: &AgentSession,
    compact_context: Option<CompactContextRecord>,
) {
    let mut session_compact = session.compact_context.write().await;
    *session_compact = compact_context;
}

/// Only update the in-memory compact context when a loaded record is present.
pub async fn set_compact_context_if_loaded(
    session: &AgentSession,
    compact_context: Option<CompactContextRecord>,
) {
    if let Some(record) = compact_context {
        let mut session_compact = session.compact_context.write().await;
        *session_compact = Some(record);
    }
}
