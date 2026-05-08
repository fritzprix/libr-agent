use crate::entity::{
    knowledge_chunk_entity, knowledge_chunk_v2, knowledge_entity, knowledge_relationship,
};
use crate::repositories::DbError;
use async_trait::async_trait;
use sea_orm::*;

use super::cleanup::{delete_chunk_global, delete_chunks_atomic};
use super::contracts::{
    KnowledgeChunkDetail, KnowledgeChunkPage, KnowledgeDeleteSummary, KnowledgeListCursor,
    KnowledgeV2Repository,
};
use super::detail::get_chunk_detail;
use super::graph::get_graph_context;
use super::search::search_hybrid;

#[derive(Debug)]
pub struct SqliteKnowledgeV2Repository {
    db: DatabaseConnection,
}

pub(super) fn build_literal_fts_query(query: &str) -> Option<String> {
    let terms = query
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

pub(super) fn embedding_bytes(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(embedding));
    for value in embedding {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub(super) fn row_to_chunk_model(row: &QueryResult) -> Result<knowledge_chunk_v2::Model, DbError> {
    Ok(knowledge_chunk_v2::Model {
        id: row.try_get("", "id").map_err(DbError::SeaOrmQueryFailed)?,
        assistant_id: row
            .try_get("", "assistant_id")
            .map_err(DbError::SeaOrmQueryFailed)?,
        content: row
            .try_get("", "content")
            .map_err(DbError::SeaOrmQueryFailed)?,
        tags: row
            .try_get("", "tags")
            .map_err(DbError::SeaOrmQueryFailed)?,
        source: row
            .try_get("", "source")
            .map_err(DbError::SeaOrmQueryFailed)?,
        created_at: row
            .try_get("", "created_at")
            .map_err(DbError::SeaOrmQueryFailed)?,
    })
}

impl SqliteKnowledgeV2Repository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl KnowledgeV2Repository for SqliteKnowledgeV2Repository {
    async fn record_chunk(
        &self,
        assistant_id: String,
        content: String,
        tags: Option<String>,
        source: Option<String>,
        embedding: Vec<f32>,
    ) -> Result<i32, DbError> {
        let now = chrono::Utc::now().timestamp_millis();

        let chunk = knowledge_chunk_v2::ActiveModel {
            id: NotSet,
            assistant_id: Set(assistant_id),
            content: Set(content),
            tags: Set(tags),
            source: Set(source),
            created_at: Set(now),
        };

        let txn = self.db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;

        let insert_result = knowledge_chunk_v2::Entity::insert(chunk)
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        let rowid = insert_result.last_insert_id;
        let embedding_bytes = embedding_bytes(&embedding);

        txn.execute(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            "INSERT INTO knowledge_vectors(rowid, embedding) VALUES (?, ?)",
            [rowid.into(), embedding_bytes.into()],
        ))
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

        txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;

        Ok(rowid)
    }

    async fn search_hybrid(
        &self,
        assistant_id: &str,
        text_query: Option<&str>,
        query_embedding: Option<Vec<f32>>,
        limit: u64,
    ) -> Result<Vec<(knowledge_chunk_v2::Model, f32)>, DbError> {
        search_hybrid(&self.db, assistant_id, text_query, query_embedding, limit).await
    }

    async fn delete_chunk(&self, id: i32, assistant_id: &str) -> Result<(), DbError> {
        let txn = self.db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;

        let result = knowledge_chunk_v2::Entity::delete_many()
            .filter(knowledge_chunk_v2::Column::Id.eq(id))
            .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if result.rows_affected == 0 {
            return Err(DbError::NotFound(format!("Chunk {} not found", id)));
        }

        txn.execute(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            "DELETE FROM knowledge_vectors WHERE rowid = ?",
            [id.into()],
        ))
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

        knowledge_chunk_entity::Entity::delete_many()
            .filter(knowledge_chunk_entity::Column::ChunkId.eq(id))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;
        Ok(())
    }

    async fn delete_chunks_atomic(&self, ids: &[i32], assistant_id: &str) -> Result<(), DbError> {
        delete_chunks_atomic(&self.db, ids, assistant_id).await
    }

    async fn delete_chunk_global(&self, id: i32) -> Result<KnowledgeDeleteSummary, DbError> {
        delete_chunk_global(&self.db, id).await
    }

    async fn list_chunks(
        &self,
        assistant_id: Option<&str>,
        query: Option<&str>,
        cursor: Option<KnowledgeListCursor>,
        limit: u64,
    ) -> Result<KnowledgeChunkPage, DbError> {
        let normalized_limit = limit.clamp(1, 500);
        let mut condition = Condition::all();

        if let Some(assistant_id) = assistant_id.filter(|value| !value.is_empty()) {
            condition = condition.add(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id));
        }

        if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
            let like_query = format!("%{}%", query);
            condition = condition.add(
                Condition::any()
                    .add(knowledge_chunk_v2::Column::Content.like(like_query.clone()))
                    .add(knowledge_chunk_v2::Column::Tags.like(like_query.clone()))
                    .add(knowledge_chunk_v2::Column::Source.like(like_query)),
            );
        }

        if let Some(cursor) = cursor {
            condition = condition.add(
                Condition::any()
                    .add(knowledge_chunk_v2::Column::CreatedAt.lt(cursor.created_at))
                    .add(
                        Condition::all()
                            .add(knowledge_chunk_v2::Column::CreatedAt.eq(cursor.created_at))
                            .add(knowledge_chunk_v2::Column::Id.lt(cursor.id)),
                    ),
            );
        }

        let mut items = knowledge_chunk_v2::Entity::find()
            .filter(condition)
            .order_by_desc(knowledge_chunk_v2::Column::CreatedAt)
            .order_by_desc(knowledge_chunk_v2::Column::Id)
            .limit(normalized_limit + 1)
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        let next_cursor = if items.len() > normalized_limit as usize {
            items.truncate(normalized_limit as usize);
            items.last().map(|item| KnowledgeListCursor {
                created_at: item.created_at,
                id: item.id,
            })
        } else {
            None
        };

        Ok(KnowledgeChunkPage { items, next_cursor })
    }

    async fn list_assistant_ids(&self) -> Result<Vec<String>, DbError> {
        knowledge_chunk_v2::Entity::find()
            .select_only()
            .column(knowledge_chunk_v2::Column::AssistantId)
            .distinct()
            .order_by_asc(knowledge_chunk_v2::Column::AssistantId)
            .into_tuple::<String>()
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)
    }

    async fn upsert_entity(
        &self,
        assistant_id: String,
        name: String,
        entity_type: Option<String>,
        description: Option<String>,
    ) -> Result<i32, DbError> {
        let existing = knowledge_entity::Entity::find()
            .filter(knowledge_entity::Column::AssistantId.eq(&assistant_id))
            .filter(knowledge_entity::Column::Name.eq(&name))
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if let Some(model) = existing {
            let mut active: knowledge_entity::ActiveModel = model.into();
            if entity_type.is_some() {
                active.entity_type = Set(entity_type);
            }
            if description.is_some() {
                active.description = Set(description);
            }

            let updated = active
                .update(&self.db)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?;
            return Ok(updated.id);
        }

        let new_entity = knowledge_entity::ActiveModel {
            id: NotSet,
            assistant_id: Set(assistant_id),
            name: Set(name),
            entity_type: Set(entity_type),
            description: Set(description),
        };

        let result = knowledge_entity::Entity::insert(new_entity)
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(result.last_insert_id)
    }

    async fn create_relationship(
        &self,
        assistant_id: String,
        source_id: i32,
        target_id: i32,
        relation_type: String,
    ) -> Result<i32, DbError> {
        if let Some(existing) = knowledge_relationship::Entity::find()
            .filter(knowledge_relationship::Column::AssistantId.eq(&assistant_id))
            .filter(knowledge_relationship::Column::SourceEntityId.eq(source_id))
            .filter(knowledge_relationship::Column::TargetEntityId.eq(target_id))
            .filter(knowledge_relationship::Column::RelationType.eq(&relation_type))
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?
        {
            return Ok(existing.id);
        }

        let relationship = knowledge_relationship::ActiveModel {
            id: NotSet,
            assistant_id: Set(assistant_id),
            source_entity_id: Set(source_id),
            target_entity_id: Set(target_id),
            relation_type: Set(relation_type),
            weight: Set(1.0),
        };

        let result = knowledge_relationship::Entity::insert(relationship)
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(result.last_insert_id)
    }

    async fn link_chunk_to_entity(&self, chunk_id: i32, entity_id: i32) -> Result<(), DbError> {
        let link = knowledge_chunk_entity::ActiveModel {
            chunk_id: Set(chunk_id),
            entity_id: Set(entity_id),
        };

        knowledge_chunk_entity::Entity::insert(link)
            .on_conflict(
                sea_query::OnConflict::columns([
                    knowledge_chunk_entity::Column::ChunkId,
                    knowledge_chunk_entity::Column::EntityId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(())
    }

    async fn get_graph_context(
        &self,
        assistant_id: &str,
        entity_name: &str,
        depth: u32,
    ) -> Result<serde_json::Value, DbError> {
        get_graph_context(&self.db, assistant_id, entity_name, depth).await
    }

    async fn find_existing_chunk_ids(
        &self,
        assistant_id: &str,
        ids: &[i32],
    ) -> Result<Vec<i32>, DbError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut existing_ids = knowledge_chunk_v2::Entity::find()
            .select_only()
            .column(knowledge_chunk_v2::Column::Id)
            .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
            .filter(knowledge_chunk_v2::Column::Id.is_in(ids.iter().copied()))
            .into_tuple::<i32>()
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;
        existing_ids.sort_unstable();
        Ok(existing_ids)
    }

    async fn get_chunk_detail(&self, id: i32) -> Result<KnowledgeChunkDetail, DbError> {
        get_chunk_detail(&self.db, id).await
    }
}
