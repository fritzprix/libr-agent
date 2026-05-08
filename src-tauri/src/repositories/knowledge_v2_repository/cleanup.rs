use crate::entity::{
    knowledge_chunk_entity, knowledge_chunk_v2, knowledge_entity, knowledge_relationship,
};
use crate::repositories::DbError;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QuerySelect, Statement, TransactionTrait,
};
use std::collections::HashSet;

use super::contracts::KnowledgeDeleteSummary;

async fn delete_chunk_vector(txn: &DatabaseTransaction, id: i32) -> Result<(), DbError> {
    txn.execute(Statement::from_sql_and_values(
        txn.get_database_backend(),
        "DELETE FROM knowledge_vectors WHERE rowid = ?",
        [id.into()],
    ))
    .await
    .map_err(DbError::SeaOrmQueryFailed)?;

    Ok(())
}

async fn find_orphan_entity_ids(
    txn: &DatabaseTransaction,
    affected_entity_ids: &HashSet<i32>,
) -> Result<Vec<i32>, DbError> {
    if affected_entity_ids.is_empty() {
        return Ok(Vec::new());
    }

    let affected_entity_id_list = affected_entity_ids.iter().copied().collect::<Vec<_>>();
    let remaining_entity_ids = knowledge_chunk_entity::Entity::find()
        .select_only()
        .column(knowledge_chunk_entity::Column::EntityId)
        .filter(knowledge_chunk_entity::Column::EntityId.is_in(affected_entity_id_list.clone()))
        .group_by(knowledge_chunk_entity::Column::EntityId)
        .into_tuple::<i32>()
        .all(txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?
        .into_iter()
        .collect::<HashSet<_>>();

    Ok(affected_entity_id_list
        .into_iter()
        .filter(|entity_id| !remaining_entity_ids.contains(entity_id))
        .collect::<Vec<_>>())
}

async fn delete_orphan_graph_state(
    txn: &DatabaseTransaction,
    assistant_id: &str,
    orphan_entity_ids: Vec<i32>,
) -> Result<(u64, u64), DbError> {
    if orphan_entity_ids.is_empty() {
        return Ok((0, 0));
    }

    let orphan_entity_count = orphan_entity_ids.len() as u64;
    let deleted_relationships = knowledge_relationship::Entity::delete_many()
        .filter(knowledge_relationship::Column::AssistantId.eq(assistant_id))
        .filter(
            Condition::any()
                .add(
                    knowledge_relationship::Column::SourceEntityId.is_in(orphan_entity_ids.clone()),
                )
                .add(
                    knowledge_relationship::Column::TargetEntityId.is_in(orphan_entity_ids.clone()),
                ),
        )
        .exec(txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    knowledge_entity::Entity::delete_many()
        .filter(knowledge_entity::Column::Id.is_in(orphan_entity_ids))
        .exec(txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    Ok((orphan_entity_count, deleted_relationships.rows_affected))
}

pub(super) async fn delete_chunks_atomic(
    db: &DatabaseConnection,
    ids: &[i32],
    assistant_id: &str,
) -> Result<(), DbError> {
    if ids.is_empty() {
        return Ok(());
    }

    let txn = db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;
    let requested_ids = ids.to_vec();
    let expected_count = requested_ids.len() as u64;

    let existing_count = knowledge_chunk_v2::Entity::find()
        .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
        .filter(knowledge_chunk_v2::Column::Id.is_in(requested_ids.clone()))
        .count(&txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    if existing_count != expected_count {
        return Err(DbError::NotFound(format!(
            "One or more chunks are missing for assistant '{}'",
            assistant_id
        )));
    }

    let delete_result = knowledge_chunk_v2::Entity::delete_many()
        .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
        .filter(knowledge_chunk_v2::Column::Id.is_in(requested_ids.clone()))
        .exec(&txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    if delete_result.rows_affected != expected_count {
        return Err(DbError::NotFound(format!(
            "One or more chunks changed while deleting for assistant '{}'",
            assistant_id
        )));
    }

    let vector_delete_sql = format!(
        "DELETE FROM knowledge_vectors WHERE rowid IN ({})",
        vec!["?"; requested_ids.len()].join(", ")
    );
    let vector_delete_values = requested_ids
        .iter()
        .copied()
        .map(Into::into)
        .collect::<Vec<sea_orm::Value>>();
    txn.execute(Statement::from_sql_and_values(
        txn.get_database_backend(),
        vector_delete_sql,
        vector_delete_values,
    ))
    .await
    .map_err(DbError::SeaOrmQueryFailed)?;

    knowledge_chunk_entity::Entity::delete_many()
        .filter(knowledge_chunk_entity::Column::ChunkId.is_in(requested_ids))
        .exec(&txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;
    Ok(())
}

pub(super) async fn delete_chunk_global(
    db: &DatabaseConnection,
    id: i32,
) -> Result<KnowledgeDeleteSummary, DbError> {
    let txn = db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;

    let chunk = knowledge_chunk_v2::Entity::find_by_id(id)
        .one(&txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?
        .ok_or_else(|| DbError::NotFound(format!("Chunk {} not found", id)))?;

    let chunk_links = knowledge_chunk_entity::Entity::find()
        .filter(knowledge_chunk_entity::Column::ChunkId.eq(id))
        .all(&txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;
    let affected_entity_ids = chunk_links
        .iter()
        .map(|link| link.entity_id)
        .collect::<HashSet<_>>();

    let delete_result = knowledge_chunk_v2::Entity::delete_many()
        .filter(knowledge_chunk_v2::Column::Id.eq(id))
        .exec(&txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    if delete_result.rows_affected == 0 {
        return Err(DbError::NotFound(format!("Chunk {} not found", id)));
    }

    delete_chunk_vector(&txn, id).await?;

    knowledge_chunk_entity::Entity::delete_many()
        .filter(knowledge_chunk_entity::Column::ChunkId.eq(id))
        .exec(&txn)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    let orphan_entity_ids = find_orphan_entity_ids(&txn, &affected_entity_ids).await?;
    let (orphan_entity_count, orphan_relationship_count) =
        delete_orphan_graph_state(&txn, &chunk.assistant_id, orphan_entity_ids).await?;

    txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;

    Ok(KnowledgeDeleteSummary {
        orphan_entity_count,
        orphan_relationship_count,
    })
}
