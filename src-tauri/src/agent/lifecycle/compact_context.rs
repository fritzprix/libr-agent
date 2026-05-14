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

pub async fn replace_compact_context(
    session: &AgentSession,
    compact_context: Option<CompactContextRecord>,
) {
    let mut session_compact = session.compact_context.write().await;
    *session_compact = compact_context;
}

pub async fn apply_compact_context_if_present(
    session: &AgentSession,
    compact_context: Option<CompactContextRecord>,
) {
    if let Some(record) = compact_context {
        let mut session_compact = session.compact_context.write().await;
        *session_compact = Some(record);
    }
}
