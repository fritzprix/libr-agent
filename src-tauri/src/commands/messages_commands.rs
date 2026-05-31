use crate::agent::session_manager::AgentSessionManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::{
    MessagePaginationCursor as RepositoryMessageCursor, MessageSlicePage as RepositoryMessageSlice,
};
use crate::repositories::MessageRepository;
use crate::search::message_index::SearchResult;
use crate::services::MessageService;
use crate::state::get_message_repository;
use crate::utils::pagination::Page;
use serde::{Deserialize, Serialize};
use tauri::command;
use tauri::State;

// The database layer has been migrated to repositories/message_repository.rs
// All database operations now go through the MessageRepository trait

// ========================================
// Tauri Commands
// ========================================

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageCursor {
    pub created_at: i64,
    pub row_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSlice {
    pub items: Vec<Message>,
    pub has_more_before: bool,
    pub oldest_cursor: Option<MessageCursor>,
}

impl From<RepositoryMessageCursor> for MessageCursor {
    fn from(value: RepositoryMessageCursor) -> Self {
        Self {
            created_at: value.created_at,
            row_id: value.row_id,
        }
    }
}

impl From<RepositoryMessageSlice> for MessageSlice {
    fn from(value: RepositoryMessageSlice) -> Self {
        Self {
            items: value.items,
            has_more_before: value.has_more_before,
            oldest_cursor: value.oldest_cursor.map(Into::into),
        }
    }
}

/// Get a paginated list of messages for a session.
#[command]
pub async fn messages_get_page(
    session_id: String,
    page: u64,
    page_size: u64,
) -> Result<Page<Message>, String> {
    let repo = get_message_repository();
    repo.get_page(&session_id, page, page_size)
        .await
        .map_err(|e| e.to_string())
}

/// Get messages older than a row-id cursor for a session in ascending causal order.
#[command]
pub async fn messages_get_messages_before(
    session_id: String,
    before_row_id: i64,
    limit: u64,
) -> Result<MessageSlice, String> {
    let repo = get_message_repository();
    let slice = repo
        .get_messages_before(&session_id, before_row_id, limit)
        .await
        .map_err(|e| e.to_string())?;

    Ok(slice.into())
}

/// Insert or update multiple messages at once.
#[command]
pub async fn messages_upsert_many(messages: Vec<Message>) -> Result<(), String> {
    let repo = get_message_repository();
    repo.insert_many(messages).await.map_err(|e| e.to_string())
}

/// Insert or update a single message.
#[command]
pub async fn messages_upsert(message: Message) -> Result<(), String> {
    let repo = get_message_repository();
    repo.insert(&message).await.map_err(|e| e.to_string())
}

/// Delete a single message by ID.
/// Also removes the message from the in-memory cache of active sessions to maintain consistency.
#[command]
pub async fn messages_delete(
    message_id: String,
    session_manager: State<'_, AgentSessionManager>,
) -> Result<(), String> {
    MessageService::delete_message(message_id, &session_manager).await
}

/// Delete all messages for a specific session.
#[command]
pub async fn messages_delete_all_for_session(session_id: String) -> Result<(), String> {
    let repo = get_message_repository();
    repo.delete_by_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

// ========================================
// Search Functionality
// ========================================

/// Search messages using BM25 full-text search.
///
/// # Arguments
/// * `query` - Search query string
/// * `session_id` - Optional session ID to search within (if None, searches all sessions)
/// * `page` - Page number (1-indexed)
/// * `page_size` - Number of results per page
///
/// # Returns
/// Paginated search results with relevance scores
#[command]
pub async fn messages_search(
    query: String,
    session_id: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<Page<SearchResult>, String> {
    MessageService::search_messages(query, session_id, page, page_size).await
}
