use sea_orm::{
    sea_query::Expr, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, QueryFilter, QuerySelect, Set, Statement,
};

use crate::entity::prelude::{Message as MessageEntity, MessageIndexMeta};
use crate::entity::{message, message_index_meta};

use super::super::error::DbError;

pub(super) async fn update_index_meta(
    db: &DatabaseConnection,
    session_id: &str,
    index_path: &str,
    doc_count: usize,
    rebuild_duration_ms: i64,
) -> Result<(), DbError> {
    use sea_orm::sea_query::OnConflict;
    let now = chrono::Utc::now().timestamp_millis();

    let model = message_index_meta::ActiveModel {
        session_id: Set(session_id.to_string()),
        index_path: Set(Some(index_path.to_string())),
        last_indexed_at: Set(now),
        doc_count: Set(doc_count as i32),
        index_version: Set(1),
        last_rebuild_duration_ms: Set(Some(rebuild_duration_ms)),
    };

    MessageIndexMeta::insert(model)
        .on_conflict(
            OnConflict::column(message_index_meta::Column::SessionId)
                .update_columns([
                    message_index_meta::Column::IndexPath,
                    message_index_meta::Column::LastIndexedAt,
                    message_index_meta::Column::DocCount,
                    message_index_meta::Column::LastRebuildDurationMs,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(())
}

pub(super) async fn get_last_indexed_at(
    db: &DatabaseConnection,
    session_id: &str,
) -> Result<i64, DbError> {
    let result = MessageIndexMeta::find_by_id(session_id).one(db).await?;

    Ok(result.map(|m| m.last_indexed_at).unwrap_or(0))
}

pub(super) async fn is_index_dirty(
    db: &DatabaseConnection,
    session_id: &str,
) -> Result<bool, DbError> {
    let last_indexed_at = get_last_indexed_at(db, session_id).await?;

    let max_created: Option<i64> = MessageEntity::find()
        .filter(message::Column::SessionId.eq(session_id))
        .select_only()
        .column_as(Expr::col(message::Column::CreatedAt).max(), "max_created")
        .into_tuple()
        .one(db)
        .await?;

    Ok(max_created.map(|t| t > last_indexed_at).unwrap_or(false))
}

/// Sessions whose newest message is newer than the last index build.
///
/// One grouped scan replaces the background worker's per-session pair of
/// `message_index_meta` + `MAX(created_at)` queries. `sessions.last_message_at`
/// is not the dirty signal: that column was added nullable and was never
/// backfilled, so legacy sessions would be skipped.
pub(super) async fn get_dirty_session_ids(db: &DatabaseConnection) -> Result<Vec<String>, DbError> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT m.session_id AS session_id \
             FROM messages AS m \
             LEFT JOIN message_index_meta AS meta ON meta.session_id = m.session_id \
             GROUP BY m.session_id \
             HAVING MAX(m.created_at) > COALESCE(MAX(meta.last_indexed_at), 0)"
                .to_string(),
        ))
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    rows.into_iter()
        .map(|row| {
            row.try_get("", "session_id")
                .map_err(DbError::SeaOrmQueryFailed)
        })
        .collect()
}

pub(super) async fn delete_index_metadata(
    db: &DatabaseConnection,
    session_id: &str,
) -> Result<(), DbError> {
    MessageIndexMeta::delete_by_id(session_id).exec(db).await?;
    Ok(())
}
