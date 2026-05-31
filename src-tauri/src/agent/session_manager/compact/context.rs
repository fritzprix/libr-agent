use super::summary::extract_message_preview;
use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use crate::repositories::{CompactContextRecord, CompactContextRepository, MessageRepository};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactContextView {
    pub id: String,
    pub session_id: String,
    pub to_id: String,
    pub summary: String,
    pub created_at: i64,
    pub latest_included_preview: Option<String>,
    pub condensed_count: Option<usize>,
}

pub(super) async fn compacted_messages_prefix_for_to_id(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    to_id: &str,
) -> Vec<Message> {
    let active = active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Vec::new();
    };
    let session_messages = session.messages.read().await;
    let Some(to_idx) = session_messages
        .iter()
        .position(|message| message.id == to_id)
    else {
        return Vec::new();
    };
    session_messages.iter().take(to_idx + 1).cloned().collect()
}

pub async fn get_compact_context(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<Option<CompactContextRecord>, String> {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        let compact = session.compact_context.read().await;
        if compact.is_some() {
            return Ok((*compact).clone());
        }
    }

    let repo = crate::state::get_compact_context_repository();
    repo.get_by_session_id(session_id)
        .await
        .map_err(|e| e.to_string())
}

async fn load_boundary_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    record: &CompactContextRecord,
) -> Result<HashMap<String, Message>, String> {
    let mut boundary_messages = HashMap::new();

    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let messages = session.messages.read().await;
            for message in messages.iter() {
                if message.id == record.to_id {
                    boundary_messages.insert(message.id.clone(), message.clone());
                }
            }
        }
    }

    let mut missing_ids = Vec::new();
    if !boundary_messages.contains_key(&record.to_id) {
        missing_ids.push(record.to_id.clone());
    }

    if !missing_ids.is_empty() {
        let repo = crate::state::get_message_repository();
        let loaded = repo
            .get_by_ids(missing_ids)
            .await
            .map_err(|error| error.to_string())?;
        for message in loaded {
            boundary_messages.insert(message.id.clone(), message);
        }
    }

    Ok(boundary_messages)
}

fn build_compact_context_view(
    record: CompactContextRecord,
    boundary_messages: &HashMap<String, Message>,
) -> CompactContextView {
    let latest_included_preview = boundary_messages
        .get(&record.to_id)
        .and_then(extract_message_preview);

    CompactContextView {
        id: record.id,
        session_id: record.session_id,
        to_id: record.to_id,
        summary: record.summary,
        created_at: record.created_at,
        latest_included_preview,
        condensed_count: record.condensed_count,
    }
}

pub async fn get_compact_context_view(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<Option<CompactContextView>, String> {
    let Some(record) = get_compact_context(active_sessions, session_id).await? else {
        return Ok(None);
    };

    let boundary_messages = load_boundary_messages(active_sessions, session_id, &record).await?;
    Ok(Some(build_compact_context_view(record, &boundary_messages)))
}

pub async fn save_compact_context(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    record: CompactContextRecord,
) -> Result<(), String> {
    let repo = crate::state::get_compact_context_repository();
    repo.upsert(&record).await.map_err(|e| e.to_string())?;

    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let mut compact = session.compact_context.write().await;
            *compact = Some(record.clone());
        }
    }

    Ok(())
}

pub async fn wait_for_compaction_to_settle(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();

    loop {
        let is_settled = {
            let active = active_sessions.read().await;
            let session = active
                .get(session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            session.compaction.is_settled()
        };

        if is_settled {
            return Ok(());
        }

        if started_at.elapsed() >= timeout {
            return Err(format!(
                "Timed out waiting for compaction to settle for session {} after {} seconds",
                session_id,
                timeout.as_secs()
            ));
        }

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}
