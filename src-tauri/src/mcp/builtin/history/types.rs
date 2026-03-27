use serde::Serialize;

use crate::utils::pagination::Page;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionItem {
    pub session_id: String,
    pub name: Option<String>,
    pub status: String,
    pub agent_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub lineage_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_message_at: Option<i64>,
    pub message_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMessageListItem {
    pub message_id: String,
    pub role: String,
    pub created_at: i64,
    pub content_preview: String,
    pub content_length: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionReadResponse {
    pub session: HistorySessionItem,
    pub messages: Page<HistoryMessageListItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMessageReadResponse {
    pub message_id: String,
    pub session_id: String,
    pub role: String,
    pub created_at: i64,
    pub total_chars: usize,
    pub chunk_offset: usize,
    pub chunk_length: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub content_chunk: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySearchMatch {
    pub session_id: String,
    pub message_id: String,
    pub role: String,
    pub created_at: i64,
    pub score: f32,
    pub snippet: String,
    pub content_length: usize,
}
