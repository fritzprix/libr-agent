//! Session analysis export (Markdown + ATIF-v1.7).
//!
//! Loads the persisted SQLite snapshot for one session and writes a shareable
//! file. Compaction, recovery, and other synthetic scaffolding messages are
//! excluded so the export reflects the agent trajectory, not internal overlays.

mod atif;
mod markdown;

use crate::commands::download_commands::save_bytes_via_dialog;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::session_repository::SessionRepository;
use crate::state::{get_message_repository, get_session_repository};
use serde::Deserialize;

pub use atif::{build_atif_trajectory, AtifAgentMetadata};

const EXPORT_MESSAGE_PAGE_SIZE: u64 = 500;
const AGENT_EXPORT_NAME: &str = "LibrAgent";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionExportFormat {
    Markdown,
    Atif,
}

/// True when a persisted message is internal scaffolding, not agent analysis.
pub fn is_excluded_from_session_analysis_export(message: &Message) -> bool {
    message.is_compact_summary()
        || message.is_compaction_instruction()
        || message.is_recovery_message()
        || message.is_internal_synthetic_user_message()
        || message.is_request_layout_scaffolding_message()
        || message.is_compaction_overlay_message()
}

pub fn filter_session_analysis_messages(messages: Vec<Message>) -> Vec<Message> {
    messages
        .into_iter()
        .filter(|message| !is_excluded_from_session_analysis_export(message))
        .collect()
}

/// Page through SQLite in persisted causal order (`rowid ASC`).
pub async fn load_session_messages(
    message_repo: &impl MessageRepository,
    session_id: &str,
) -> Result<Vec<Message>, String> {
    let mut page = 1u64;
    let mut messages = Vec::new();

    loop {
        let batch = message_repo
            .get_page(session_id, page, EXPORT_MESSAGE_PAGE_SIZE)
            .await
            .map_err(|e| format!("Failed to get messages for session {session_id}: {e}"))?;

        if batch.items.is_empty() {
            break;
        }

        messages.extend(batch.items);

        if !batch.has_next_page {
            break;
        }

        page += 1;
    }

    Ok(messages)
}

pub fn export_file_name(
    session_name: Option<&str>,
    session_id: &str,
    format: SessionExportFormat,
) -> String {
    let stem = session_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(session_id);
    let stem: String = stem
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '\0' => '_',
            _ => ch,
        })
        .collect();
    match format {
        SessionExportFormat::Markdown => format!("{stem}.md"),
        SessionExportFormat::Atif => format!("{stem}_trajectory.json"),
    }
}

fn atif_model_name(provider: &str, model: &str) -> Option<String> {
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() && model.is_empty() {
        return None;
    }
    if provider.is_empty() {
        return Some(model.to_string());
    }
    if model.is_empty() {
        return Some(provider.to_string());
    }
    Some(format!("{provider}/{model}"))
}

/// Build file bytes for a session snapshot. Compaction/recovery messages are dropped.
pub fn render_session_export(
    messages: Vec<Message>,
    metadata: AtifAgentMetadata,
    format: SessionExportFormat,
) -> Result<Vec<u8>, String> {
    let filtered = filter_session_analysis_messages(messages);
    match format {
        SessionExportFormat::Markdown => Ok(markdown::messages_to_markdown(&filtered).into_bytes()),
        SessionExportFormat::Atif => {
            let trajectory = build_atif_trajectory(&filtered, metadata);
            let json = serde_json::to_string_pretty(&trajectory)
                .map_err(|e| format!("Failed to serialize ATIF trajectory: {e}"))?;
            Ok(json.into_bytes())
        }
    }
}

/// Load the current SQLite snapshot and prompt the native save dialog.
pub async fn export_session_file(
    app_handle: tauri::AppHandle,
    session_id: String,
    format: SessionExportFormat,
) -> Result<String, String> {
    if session_id.trim().is_empty() {
        return Err("sessionId is required".to_string());
    }

    let session_repo = get_session_repository();
    let message_repo = get_message_repository();

    let session = session_repo
        .get_session(&session_id)
        .await
        .map_err(|e| format!("Failed to load session {session_id}: {e}"))?
        .ok_or_else(|| format!("Session not found: {session_id}"))?;

    let messages = load_session_messages(message_repo, &session_id).await?;
    let metadata = AtifAgentMetadata {
        name: AGENT_EXPORT_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        model_name: atif_model_name(&session.provider, &session.model),
        session_id: Some(session.id.clone()),
    };
    let bytes = render_session_export(messages, metadata, format)?;
    let file_name = export_file_name(session.name.as_deref(), &session.id, format);
    save_bytes_via_dialog(app_handle, file_name, bytes).await
}
