use sea_orm::{
    sea_query::Expr, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryResult, Set,
};

use crate::entity::message;
use crate::entity::session;
use crate::models::chat::{Message, MessageSource};
use crate::utils::json::{from_json_option, from_json_or_default, to_json_option};

use super::super::error::DbError;
use super::types::{MessagePaginationCursor, MessageRowWithCursor, MessageSlicePage};

pub(super) fn row_to_message_model(row: &QueryResult) -> Result<message::Model, DbError> {
    Ok(message::Model {
        id: row.try_get("", "id").map_err(DbError::SeaOrmQueryFailed)?,
        session_id: row
            .try_get("", "session_id")
            .map_err(DbError::SeaOrmQueryFailed)?,
        role: row
            .try_get("", "role")
            .map_err(DbError::SeaOrmQueryFailed)?,
        content: row
            .try_get("", "content")
            .map_err(DbError::SeaOrmQueryFailed)?,
        tool_calls: row
            .try_get("", "tool_calls")
            .map_err(DbError::SeaOrmQueryFailed)?,
        tool_call_id: row
            .try_get("", "tool_call_id")
            .map_err(DbError::SeaOrmQueryFailed)?,
        is_streaming: row
            .try_get("", "is_streaming")
            .map_err(DbError::SeaOrmQueryFailed)?,
        thinking: row
            .try_get("", "thinking")
            .map_err(DbError::SeaOrmQueryFailed)?,
        thinking_signature: row
            .try_get("", "thinking_signature")
            .map_err(DbError::SeaOrmQueryFailed)?,
        assistant_id: row
            .try_get("", "assistant_id")
            .map_err(DbError::SeaOrmQueryFailed)?,
        attachments: row
            .try_get("", "attachments")
            .map_err(DbError::SeaOrmQueryFailed)?,
        tool_use: row
            .try_get("", "tool_use")
            .map_err(DbError::SeaOrmQueryFailed)?,
        created_at: row
            .try_get("", "created_at")
            .map_err(DbError::SeaOrmQueryFailed)?,
        updated_at: row
            .try_get("", "updated_at")
            .map_err(DbError::SeaOrmQueryFailed)?,
        source: row
            .try_get("", "source")
            .map_err(DbError::SeaOrmQueryFailed)?,
        error: row
            .try_get("", "error")
            .map_err(DbError::SeaOrmQueryFailed)?,
        usage: row
            .try_get("", "usage")
            .map_err(DbError::SeaOrmQueryFailed)?,
        prompt_tokens: row
            .try_get("", "prompt_tokens")
            .map_err(DbError::SeaOrmQueryFailed)?,
    })
}

pub(super) fn row_to_cursor(row: &QueryResult) -> Result<MessagePaginationCursor, DbError> {
    Ok(MessagePaginationCursor {
        created_at: row
            .try_get("", "created_at")
            .map_err(DbError::SeaOrmQueryFailed)?,
        row_id: row
            .try_get("", "cursor_rowid")
            .map_err(DbError::SeaOrmQueryFailed)?,
    })
}

pub(super) fn row_to_message_with_cursor(
    row: &QueryResult,
) -> Result<MessageRowWithCursor, DbError> {
    Ok(MessageRowWithCursor {
        model: row_to_message_model(row)?,
        cursor: row_to_cursor(row)?,
    })
}

pub(super) fn validate_slice_limit(limit: u64) -> Result<i64, DbError> {
    if limit == 0 {
        return Err(DbError::InvalidInput(
            "Message slice limit must be greater than zero".to_string(),
        ));
    }

    i64::try_from(limit.saturating_add(1))
        .map_err(|_| DbError::InvalidInput("Message slice limit is too large".to_string()))
}

pub(super) fn build_slice_page(
    mut rows: Vec<MessageRowWithCursor>,
    limit: u64,
) -> Result<MessageSlicePage, DbError> {
    let has_more_before = rows.len() as u64 > limit;
    if has_more_before {
        rows.truncate(limit as usize);
    }
    rows.reverse();

    let oldest_cursor = rows.first().map(|row| row.cursor.clone());
    let items = rows
        .into_iter()
        .map(|row| model_to_message(row.model))
        .collect();

    Ok(MessageSlicePage {
        items,
        has_more_before,
        oldest_cursor,
    })
}

/// Rebuild UI-facing metadata / error fields from persisted columns.
///
/// `messages.metadata` is not a DB column. Tool UI state is persisted in the
/// `error` column as a JSON envelope:
/// - `{"toolError": true}` — tool failure marker (also legacy `content[].isError`)
/// - `{"structuredContent": ...}` — MCP structured_content for chat UI cards
/// - both keys may appear together
///
/// When the column only holds this envelope, `Message.error` stays `None`
/// (that field is reserved for LLM/service failure UI payloads).
pub(super) fn decode_persisted_tool_error(
    content_json: &str,
    error: Option<serde_json::Value>,
) -> (Option<serde_json::Value>, Option<serde_json::Value>) {
    let tool_error_from_legacy_content =
        crate::mcp::types::raw_content_json_has_legacy_item_error(content_json);

    let envelope = error.as_ref().and_then(|value| value.as_object());
    let tool_error_from_column = envelope
        .and_then(|object| object.get("toolError"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let structured_content = envelope
        .and_then(|object| object.get("structuredContent"))
        .cloned();
    let is_ui_envelope = tool_error_from_column || structured_content.is_some();

    let tool_error = tool_error_from_column || tool_error_from_legacy_content;

    let metadata = {
        let mut map = serde_json::Map::new();
        if tool_error {
            map.insert("toolError".to_string(), serde_json::Value::Bool(true));
        }
        if let Some(structured) = structured_content {
            map.insert("structuredContent".to_string(), structured);
        }
        if map.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(map))
        }
    };

    // UI envelope occupies the error column; do not expose it as Message.error.
    let error = if is_ui_envelope { None } else { error };

    (error, metadata)
}

/// Encode in-memory tool UI metadata into the durable `error` column envelope.
///
/// Persists `metadata.toolError` and/or `metadata.structuredContent` so chat
/// structured cards survive session reload. Falls back to `message.error` for
/// real LLM/service failure payloads when no UI envelope keys are present.
pub(super) fn encode_persisted_error(message: &Message) -> Option<serde_json::Value> {
    let tool_error = message
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolError"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let structured_content = message
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("structuredContent"))
        .cloned();

    if tool_error || structured_content.is_some() {
        let mut map = serde_json::Map::new();
        if tool_error {
            map.insert("toolError".to_string(), serde_json::Value::Bool(true));
        }
        if let Some(structured) = structured_content {
            map.insert("structuredContent".to_string(), structured);
        }
        return Some(serde_json::Value::Object(map));
    }

    message.error.clone()
}

/// Convert SeaORM message model to Message type
pub(super) fn model_to_message(model: message::Model) -> Message {
    let content: Vec<crate::mcp::types::MCPContent> = from_json_or_default(&model.content);

    let tool_calls: Option<Vec<crate::agent::types::ToolCall>> =
        from_json_option(&model.tool_calls);

    let attachments: Option<serde_json::Value> = from_json_option(&model.attachments);

    let tool_use: Option<serde_json::Value> = from_json_option(&model.tool_use);

    let raw_error: Option<serde_json::Value> = from_json_option(&model.error);

    let usage: Option<serde_json::Value> = from_json_option(&model.usage);

    let (error, metadata) = decode_persisted_tool_error(&model.content, raw_error);

    Message {
        id: model.id,
        session_id: model.session_id,
        role: model.role,
        content,
        tool_calls,
        tool_call_id: model.tool_call_id,
        is_streaming: model.is_streaming.map(|v| v != 0),
        thinking: model.thinking,
        thinking_signature: model.thinking_signature,
        assistant_id: model.assistant_id,
        attachments,
        tool_use,
        created_at: model.created_at,
        updated_at: model.updated_at,
        source: model.source.map(MessageSource::from_raw),
        error,
        usage,
        prompt_tokens: model.prompt_tokens,
        metadata,
    }
}

/// Convert Message type to SeaORM ActiveModel
pub(crate) fn message_to_active_model(message: &Message) -> Result<message::ActiveModel, DbError> {
    let content_json = serde_json::to_string(&message.content)
        .map_err(|e| DbError::SerializationError(format!("Failed to serialize content: {}", e)))?;

    let tool_calls_json = to_json_option(&message.tool_calls).map_err(|e| {
        DbError::SerializationError(format!("Failed to serialize tool_calls: {}", e))
    })?;

    let attachments_json = to_json_option(&message.attachments).map_err(|e| {
        DbError::SerializationError(format!("Failed to serialize attachments: {}", e))
    })?;

    let tool_use_json = to_json_option(&message.tool_use)
        .map_err(|e| DbError::SerializationError(format!("Failed to serialize tool_use: {}", e)))?;

    let error_json = to_json_option(&encode_persisted_error(message))
        .map_err(|e| DbError::SerializationError(format!("Failed to serialize error: {}", e)))?;

    let usage_json = to_json_option(&message.usage)
        .map_err(|e| DbError::SerializationError(format!("Failed to serialize usage: {}", e)))?;

    Ok(message::ActiveModel {
        id: Set(message.id.clone()),
        session_id: Set(message.session_id.clone()),
        role: Set(message.role.clone()),
        content: Set(content_json),
        tool_calls: Set(tool_calls_json),
        tool_call_id: Set(message.tool_call_id.clone()),
        is_streaming: Set(message.is_streaming.map(|b| if b { 1 } else { 0 })),
        thinking: Set(message.thinking.clone()),
        thinking_signature: Set(message.thinking_signature.clone()),
        assistant_id: Set(message.assistant_id.clone()),
        attachments: Set(attachments_json),
        tool_use: Set(tool_use_json),
        created_at: Set(message.created_at),
        updated_at: Set(message.updated_at),
        source: Set(message
            .source
            .as_ref()
            .map(|source| source.as_str().to_string())),
        error: Set(error_json),
        usage: Set(usage_json),
        prompt_tokens: Set(message.prompt_tokens),
    })
}

/// Helper to get the OnConflict strategy for upserting messages
pub(crate) fn get_upsert_on_conflict() -> sea_orm::sea_query::OnConflict {
    use sea_orm::sea_query::OnConflict;
    OnConflict::column(message::Column::Id)
        .update_columns([
            message::Column::SessionId,
            message::Column::Role,
            message::Column::Content,
            message::Column::ToolCalls,
            message::Column::ToolCallId,
            message::Column::IsStreaming,
            message::Column::Thinking,
            message::Column::ThinkingSignature,
            message::Column::AssistantId,
            message::Column::Attachments,
            message::Column::ToolUse,
            message::Column::UpdatedAt,
            message::Column::Source,
            message::Column::Error,
            message::Column::Usage,
            message::Column::PromptTokens,
        ])
        .to_owned()
}

pub(crate) async fn update_session_last_message_at<C>(
    db: &C,
    session_id: &str,
    last_message_at: i64,
) -> Result<(), DbError>
where
    C: ConnectionTrait,
{
    session::Entity::update_many()
        .col_expr(
            session::Column::LastMessageAt,
            Expr::cust_with_values("MAX(COALESCE(last_message_at, 0), ?)", [last_message_at]),
        )
        .filter(session::Column::Id.eq(session_id))
        .exec(db)
        .await?;

    Ok(())
}
