use crate::entity::knowledge_chunk_v2;
use crate::repositories::DbError;
use async_trait::async_trait;

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

    async fn delete_chunks_atomic(&self, ids: &[i32], assistant_id: &str) -> Result<(), DbError>;

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

    async fn find_existing_chunk_ids(
        &self,
        assistant_id: &str,
        ids: &[i32],
    ) -> Result<Vec<i32>, DbError>;

    async fn get_chunk_detail(&self, id: i32) -> Result<KnowledgeChunkDetail, DbError>;
}
