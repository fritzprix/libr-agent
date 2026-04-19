use crate::entity::{
    knowledge_chunk_entity, knowledge_chunk_v2, knowledge_entity, knowledge_relationship,
};
use crate::repositories::DbError;
use async_trait::async_trait;
use sea_orm::*;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct KnowledgeGraphEntity {
    pub id: i32,
    pub assistant_id: String,
    pub name: String,
    pub entity_type: Option<String>,
    pub description: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone)]
pub struct KnowledgeGraphRelationship {
    pub id: i32,
    pub assistant_id: String,
    pub source_entity_id: i32,
    pub target_entity_id: i32,
    pub relation_type: String,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct KnowledgeChunkDetail {
    pub chunk: knowledge_chunk_v2::Model,
    pub primary_entity_ids: Vec<i32>,
    pub entities: Vec<KnowledgeGraphEntity>,
    pub relationships: Vec<KnowledgeGraphRelationship>,
}

#[derive(Debug, Clone, Copy)]
pub struct KnowledgeDeleteSummary {
    pub orphan_entity_count: u64,
    pub orphan_relationship_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnowledgeListCursor {
    pub created_at: i64,
    pub id: i32,
}

#[derive(Debug, Clone)]
pub struct KnowledgeChunkPage {
    pub items: Vec<knowledge_chunk_v2::Model>,
    pub next_cursor: Option<KnowledgeListCursor>,
}

#[async_trait]
pub trait KnowledgeV2Repository: Send + Sync {
    async fn record_chunk(
        &self,
        assistant_id: String,
        content: String,
        tags: Option<String>,
        source: Option<String>,
        embedding: Vec<f32>,
    ) -> Result<i32, DbError>;

    async fn search_hybrid(
        &self,
        assistant_id: &str,
        text_query: Option<&str>,
        query_embedding: Option<Vec<f32>>,
        limit: u64,
    ) -> Result<Vec<(knowledge_chunk_v2::Model, f32)>, DbError>;

    async fn delete_chunk(&self, id: i32, assistant_id: &str) -> Result<(), DbError>;

    async fn delete_chunk_global(&self, id: i32) -> Result<KnowledgeDeleteSummary, DbError>;

    async fn list_chunks(
        &self,
        assistant_id: Option<&str>,
        query: Option<&str>,
        cursor: Option<KnowledgeListCursor>,
        limit: u64,
    ) -> Result<KnowledgeChunkPage, DbError>;

    async fn list_assistant_ids(&self) -> Result<Vec<String>, DbError>;

    async fn upsert_entity(
        &self,
        assistant_id: String,
        name: String,
        entity_type: Option<String>,
        description: Option<String>,
    ) -> Result<i32, DbError>;

    async fn create_relationship(
        &self,
        assistant_id: String,
        source_id: i32,
        target_id: i32,
        relation_type: String,
    ) -> Result<i32, DbError>;

    async fn link_chunk_to_entity(&self, chunk_id: i32, entity_id: i32) -> Result<(), DbError>;

    async fn get_graph_context(
        &self,
        assistant_id: &str,
        entity_name: &str,
        depth: u32,
    ) -> Result<serde_json::Value, DbError>;

    async fn get_chunk_detail(&self, id: i32) -> Result<KnowledgeChunkDetail, DbError>;
}

#[derive(Debug)]
pub struct SqliteKnowledgeV2Repository {
    db: DatabaseConnection,
}

impl SqliteKnowledgeV2Repository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn build_literal_fts_query(query: &str) -> Option<String> {
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

    fn embedding_bytes(embedding: &[f32]) -> Vec<u8> {
        // sqlite-vec expects little-endian f32 bytes in a BLOB.
        let mut bytes = Vec::with_capacity(std::mem::size_of_val(embedding));
        for value in embedding {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn row_to_chunk_model(row: &QueryResult) -> Result<knowledge_chunk_v2::Model, DbError> {
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

        let embedding_bytes = Self::embedding_bytes(&embedding);

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
        if text_query.is_none() && query_embedding.is_none() {
            return Err(DbError::InvalidInput(
                "search_hybrid requires a text query, an embedding, or both".to_string(),
            ));
        }

        let mut fused_scores = HashMap::<i32, f32>::new();
        let mut chunk_models = HashMap::<i32, knowledge_chunk_v2::Model>::new();
        const RRF_K: f32 = 60.0;

        if let Some(query) = text_query
            .map(str::trim)
            .and_then(Self::build_literal_fts_query)
        {
            let keyword_sql = r#"
                SELECT
                    c.id, c.assistant_id, c.content, c.tags, c.source, c.created_at,
                    bm25(knowledge_chunks_fts) AS bm25_rank
                FROM knowledge_chunks_fts
                JOIN knowledge_chunks_v2 c ON c.id = knowledge_chunks_fts.rowid
                WHERE c.assistant_id = ? AND knowledge_chunks_fts MATCH ?
                ORDER BY bm25_rank
                LIMIT ?
            "#;

            let keyword_results = self
                .db
                .query_all(Statement::from_sql_and_values(
                    self.db.get_database_backend(),
                    keyword_sql,
                    [assistant_id.into(), query.into(), limit.into()],
                ))
                .await
                .map_err(DbError::SeaOrmQueryFailed)?;

            for (index, row) in keyword_results.into_iter().enumerate() {
                let model = Self::row_to_chunk_model(&row)?;
                let rank = index as f32 + 1.0;
                *fused_scores.entry(model.id).or_insert(0.0) += 1.0 / (RRF_K + rank);
                chunk_models.entry(model.id).or_insert(model);
            }
        }

        if let Some(query_embedding) = query_embedding {
            let semantic_sql = r#"
                SELECT
                    c.id, c.assistant_id, c.content, c.tags, c.source, c.created_at,
                    v.distance
                FROM knowledge_chunks_v2 c
                JOIN knowledge_vectors v ON c.id = v.rowid
                WHERE c.assistant_id = ? AND v.embedding MATCH ? AND k = ?
                ORDER BY v.distance
            "#;

            let query_bytes = Self::embedding_bytes(&query_embedding);
            let semantic_results = self
                .db
                .query_all(Statement::from_sql_and_values(
                    self.db.get_database_backend(),
                    semantic_sql,
                    [assistant_id.into(), query_bytes.into(), limit.into()],
                ))
                .await
                .map_err(DbError::SeaOrmQueryFailed)?;

            for (index, row) in semantic_results.into_iter().enumerate() {
                let model = Self::row_to_chunk_model(&row)?;
                let rank = index as f32 + 1.0;
                *fused_scores.entry(model.id).or_insert(0.0) += 1.0 / (RRF_K + rank);
                chunk_models.entry(model.id).or_insert(model);
            }
        }

        let mut ranked_ids = fused_scores.into_iter().collect::<Vec<_>>();
        ranked_ids.sort_by(|(left_id, left_score), (right_id, right_score)| {
            right_score
                .partial_cmp(left_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left_id.cmp(right_id))
        });

        let mut out = Vec::new();
        for (chunk_id, score) in ranked_ids.into_iter().take(limit as usize) {
            if let Some(model) = chunk_models.remove(&chunk_id) {
                out.push((model, score));
            }
        }

        Ok(out)
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

    async fn delete_chunk(&self, id: i32, assistant_id: &str) -> Result<(), DbError> {
        let txn = self.db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;

        // Delete from metadata table
        let res = knowledge_chunk_v2::Entity::delete_many()
            .filter(knowledge_chunk_v2::Column::Id.eq(id))
            .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if res.rows_affected == 0 {
            return Err(DbError::NotFound(format!("Chunk {} not found", id)));
        }

        // Delete from vector table
        txn.execute(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            "DELETE FROM knowledge_vectors WHERE rowid = ?",
            [id.into()],
        ))
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

        // Also cleanup chunk-entity links
        knowledge_chunk_entity::Entity::delete_many()
            .filter(knowledge_chunk_entity::Column::ChunkId.eq(id))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;
        Ok(())
    }

    async fn delete_chunk_global(&self, id: i32) -> Result<KnowledgeDeleteSummary, DbError> {
        let txn = self.db.begin().await.map_err(DbError::SeaOrmQueryFailed)?;

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

        let res = knowledge_chunk_v2::Entity::delete_many()
            .filter(knowledge_chunk_v2::Column::Id.eq(id))
            .exec(&txn)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if res.rows_affected == 0 {
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

        let orphan_entity_ids = if affected_entity_ids.is_empty() {
            Vec::new()
        } else {
            let affected_entity_id_list = affected_entity_ids.iter().copied().collect::<Vec<_>>();
            let remaining_entity_ids = knowledge_chunk_entity::Entity::find()
                .select_only()
                .column(knowledge_chunk_entity::Column::EntityId)
                .filter(
                    knowledge_chunk_entity::Column::EntityId.is_in(affected_entity_id_list.clone()),
                )
                .group_by(knowledge_chunk_entity::Column::EntityId)
                .into_tuple::<i32>()
                .all(&txn)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?
                .into_iter()
                .collect::<HashSet<_>>();

            affected_entity_id_list
                .into_iter()
                .filter(|entity_id| !remaining_entity_ids.contains(entity_id))
                .collect::<Vec<_>>()
        };

        let orphan_entity_count = orphan_entity_ids.len() as u64;
        let orphan_relationship_count = if orphan_entity_ids.is_empty() {
            0
        } else {
            let deleted_relationships = knowledge_relationship::Entity::delete_many()
                .filter(knowledge_relationship::Column::AssistantId.eq(chunk.assistant_id))
                .filter(
                    Condition::any()
                        .add(
                            knowledge_relationship::Column::SourceEntityId
                                .is_in(orphan_entity_ids.clone()),
                        )
                        .add(
                            knowledge_relationship::Column::TargetEntityId
                                .is_in(orphan_entity_ids.clone()),
                        ),
                )
                .exec(&txn)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?;

            knowledge_entity::Entity::delete_many()
                .filter(knowledge_entity::Column::Id.is_in(orphan_entity_ids))
                .exec(&txn)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?;

            deleted_relationships.rows_affected
        };

        txn.commit().await.map_err(DbError::SeaOrmQueryFailed)?;

        Ok(KnowledgeDeleteSummary {
            orphan_entity_count,
            orphan_relationship_count,
        })
    }

    async fn upsert_entity(
        &self,
        assistant_id: String,
        name: String,
        entity_type: Option<String>,
        description: Option<String>,
    ) -> Result<i32, DbError> {
        // Try to find existing first
        let existing = knowledge_entity::Entity::find()
            .filter(knowledge_entity::Column::AssistantId.eq(&assistant_id))
            .filter(knowledge_entity::Column::Name.eq(&name))
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if let Some(model) = existing {
            // Update if needed
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

        // Create new
        let new_entity = knowledge_entity::ActiveModel {
            id: NotSet,
            assistant_id: Set(assistant_id),
            name: Set(name),
            entity_type: Set(entity_type),
            description: Set(description),
        };

        let res = knowledge_entity::Entity::insert(new_entity)
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(res.last_insert_id)
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

        let rel = knowledge_relationship::ActiveModel {
            id: NotSet,
            assistant_id: Set(assistant_id),
            source_entity_id: Set(source_id),
            target_entity_id: Set(target_id),
            relation_type: Set(relation_type),
            weight: Set(1.0),
        };

        let res = knowledge_relationship::Entity::insert(rel)
            .exec(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        Ok(res.last_insert_id)
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
        let depth_limit = depth.min(3); // Safety limit to prevent infinite loops or massive memory usage

        // 1. Recursive CTE to find all entity IDs within depth
        let cte_sql = r#"
            WITH RECURSIVE traverse(id, current_depth) AS (
                SELECT id, 0 FROM knowledge_entities WHERE assistant_id = ? AND name = ?
                UNION
                SELECT 
                    CASE 
                        WHEN r.source_entity_id = t.id THEN r.target_entity_id 
                        ELSE r.source_entity_id 
                    END,
                     t.current_depth + 1
                FROM knowledge_relationships r
                JOIN traverse t ON r.source_entity_id = t.id OR r.target_entity_id = t.id
                WHERE t.current_depth < ? AND r.assistant_id = ?
            )
            SELECT
                e.id,
                e.assistant_id,
                e.name,
                e.entity_type,
                e.description,
                MIN(t.current_depth) AS current_depth
            FROM knowledge_entities e
            JOIN traverse t ON e.id = t.id
            WHERE e.assistant_id = ?
            GROUP BY e.id, e.assistant_id, e.name, e.entity_type, e.description
            ORDER BY current_depth, e.id
        "#;

        let entity_rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                cte_sql,
                [
                    assistant_id.into(),
                    entity_name.into(),
                    depth_limit.into(),
                    assistant_id.into(),
                    assistant_id.into(),
                ],
            ))
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        if entity_rows.is_empty() {
            return Ok(serde_json::json!({ "error": "Entity not found" }));
        }

        let mut entities = Vec::new();
        let mut entity_ids = Vec::new();
        for row in entity_rows {
            let id: i32 = row.try_get("", "id").map_err(DbError::SeaOrmQueryFailed)?;
            entity_ids.push(id);
            entities.push(serde_json::json!({
                "id": id,
                "name": row.try_get::<String>("", "name").ok(),
                "type": row.try_get::<String>("", "entity_type").ok(),
                "description": row.try_get::<String>("", "description").ok(),
                "depth": row.try_get::<i32>("", "current_depth").ok(),
            }));
        }

        let rel_rows = knowledge_relationship::Entity::find()
            .filter(knowledge_relationship::Column::AssistantId.eq(assistant_id))
            .filter(
                Condition::all()
                    .add(knowledge_relationship::Column::SourceEntityId.is_in(entity_ids.clone()))
                    .add(knowledge_relationship::Column::TargetEntityId.is_in(entity_ids)),
            )
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        let relationships = rel_rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "source_id": r.source_entity_id,
                    "target_id": r.target_entity_id,
                    "type": r.relation_type,
                    "weight": r.weight
                })
            })
            .collect::<Vec<_>>();

        let entity_id_set = entities
            .iter()
            .filter_map(|entity| entity.get("id").and_then(|value| value.as_i64()))
            .map(|id| id as i32)
            .collect::<Vec<_>>();
        let chunk_links = if entity_id_set.is_empty() {
            Vec::new()
        } else {
            knowledge_chunk_entity::Entity::find()
                .filter(knowledge_chunk_entity::Column::EntityId.is_in(entity_id_set))
                .all(&self.db)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?
        };

        let linked_chunk_ids = chunk_links
            .iter()
            .map(|link| link.chunk_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let linked_chunks = if linked_chunk_ids.is_empty() {
            Vec::new()
        } else {
            knowledge_chunk_v2::Entity::find()
                .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
                .filter(knowledge_chunk_v2::Column::Id.is_in(linked_chunk_ids))
                .all(&self.db)
                .await
                .map_err(DbError::SeaOrmQueryFailed)?
        };
        let mut chunk_entity_ids = HashMap::<i32, Vec<i32>>::new();
        for link in chunk_links {
            chunk_entity_ids
                .entry(link.chunk_id)
                .or_default()
                .push(link.entity_id);
        }
        let linked_chunk_json = linked_chunks
            .into_iter()
            .map(|chunk| {
                let mut entity_ids = chunk_entity_ids.remove(&chunk.id).unwrap_or_default();
                entity_ids.sort_unstable();
                serde_json::json!({
                    "id": chunk.id,
                    "content": chunk.content,
                    "tags": chunk.tags,
                    "source": chunk.source,
                    "created_at": chunk.created_at,
                    "entity_ids": entity_ids,
                })
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "root_entity": entity_name,
            "max_depth": depth_limit,
            "nodes": entities,
            "edges": relationships,
            "linked_chunks": linked_chunk_json
        }))
    }

    async fn get_chunk_detail(&self, id: i32) -> Result<KnowledgeChunkDetail, DbError> {
        let chunk = knowledge_chunk_v2::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?
            .ok_or_else(|| DbError::NotFound(format!("Chunk {} not found", id)))?;

        let chunk_links = knowledge_chunk_entity::Entity::find()
            .filter(knowledge_chunk_entity::Column::ChunkId.eq(id))
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        let mut primary_entity_ids = chunk_links
            .iter()
            .map(|link| link.entity_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        primary_entity_ids.sort_unstable();

        if primary_entity_ids.is_empty() {
            return Ok(KnowledgeChunkDetail {
                chunk,
                primary_entity_ids: Vec::new(),
                entities: Vec::new(),
                relationships: Vec::new(),
            });
        }

        let relationship_rows = knowledge_relationship::Entity::find()
            .filter(knowledge_relationship::Column::AssistantId.eq(&chunk.assistant_id))
            .filter(
                Condition::any()
                    .add(
                        knowledge_relationship::Column::SourceEntityId
                            .is_in(primary_entity_ids.clone()),
                    )
                    .add(
                        knowledge_relationship::Column::TargetEntityId
                            .is_in(primary_entity_ids.clone()),
                    ),
            )
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        let mut all_entity_ids = primary_entity_ids.iter().copied().collect::<HashSet<_>>();
        for relationship in &relationship_rows {
            all_entity_ids.insert(relationship.source_entity_id);
            all_entity_ids.insert(relationship.target_entity_id);
        }

        let entity_rows = knowledge_entity::Entity::find()
            .filter(
                knowledge_entity::Column::Id.is_in(all_entity_ids.into_iter().collect::<Vec<_>>()),
            )
            .all(&self.db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        let primary_entity_set = primary_entity_ids.iter().copied().collect::<HashSet<_>>();
        let mut entities = entity_rows
            .into_iter()
            .map(|entity| KnowledgeGraphEntity {
                id: entity.id,
                assistant_id: entity.assistant_id,
                name: entity.name,
                entity_type: entity.entity_type,
                description: entity.description,
                is_primary: primary_entity_set.contains(&entity.id),
            })
            .collect::<Vec<_>>();
        entities.sort_by(|left, right| {
            right
                .is_primary
                .cmp(&left.is_primary)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.id.cmp(&right.id))
        });

        let mut relationships = relationship_rows
            .into_iter()
            .map(|relationship| KnowledgeGraphRelationship {
                id: relationship.id,
                assistant_id: relationship.assistant_id,
                source_entity_id: relationship.source_entity_id,
                target_entity_id: relationship.target_entity_id,
                relation_type: relationship.relation_type,
                weight: relationship.weight,
            })
            .collect::<Vec<_>>();
        relationships.sort_by(|left, right| {
            left.relation_type
                .cmp(&right.relation_type)
                .then_with(|| left.source_entity_id.cmp(&right.source_entity_id))
                .then_with(|| left.target_entity_id.cmp(&right.target_entity_id))
        });

        Ok(KnowledgeChunkDetail {
            chunk,
            primary_entity_ids,
            entities,
            relationships,
        })
    }
}
