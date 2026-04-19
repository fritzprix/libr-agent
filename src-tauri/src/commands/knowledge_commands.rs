use crate::mcp::builtin::knowledge::helpers::parse_db_tags;
use crate::repositories::{
    KnowledgeChunkDetail, KnowledgeGraphEntity, KnowledgeGraphRelationship, KnowledgeListCursor,
    KnowledgeV2Repository,
};
use crate::state::get_knowledge_v2_repository;
use serde::{Deserialize, Serialize};
use tauri::command;

const DEFAULT_LIST_LIMIT: u64 = 200;
const MAX_LIST_LIMIT: u64 = 500;
const PREVIEW_LENGTH: usize = 220;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListGlobalKnowledgeRequest {
    pub query: Option<String>,
    pub assistant_id: Option<String>,
    pub cursor: Option<KnowledgeListCursorDto>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeListCursorDto {
    pub created_at: i64,
    pub id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeChunkListItemDto {
    pub id: i32,
    pub assistant_id: String,
    pub preview: String,
    pub content: String,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalKnowledgeListResponse {
    pub items: Vec<KnowledgeChunkListItemDto>,
    pub assistants: Vec<String>,
    pub next_cursor: Option<KnowledgeListCursorDto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeGraphEntityDto {
    pub id: i32,
    pub assistant_id: String,
    pub name: String,
    pub entity_type: Option<String>,
    pub description: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeGraphRelationshipDto {
    pub id: i32,
    pub assistant_id: String,
    pub source_entity_id: i32,
    pub target_entity_id: i32,
    pub relation_type: String,
    pub weight: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeChunkDetailDto {
    pub id: i32,
    pub assistant_id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub created_at: i64,
    pub primary_entity_ids: Vec<i32>,
    pub entities: Vec<KnowledgeGraphEntityDto>,
    pub relationships: Vec<KnowledgeGraphRelationshipDto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteGlobalKnowledgeResponse {
    pub deleted_chunk_id: i32,
    pub orphan_entity_count: u64,
    pub orphan_relationship_count: u64,
}

fn build_preview(content: &str) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= PREVIEW_LENGTH {
        return normalized;
    }

    normalized.chars().take(PREVIEW_LENGTH).collect::<String>() + "..."
}

impl From<KnowledgeGraphEntity> for KnowledgeGraphEntityDto {
    fn from(value: KnowledgeGraphEntity) -> Self {
        Self {
            id: value.id,
            assistant_id: value.assistant_id,
            name: value.name,
            entity_type: value.entity_type,
            description: value.description,
            is_primary: value.is_primary,
        }
    }
}

impl From<KnowledgeGraphRelationship> for KnowledgeGraphRelationshipDto {
    fn from(value: KnowledgeGraphRelationship) -> Self {
        Self {
            id: value.id,
            assistant_id: value.assistant_id,
            source_entity_id: value.source_entity_id,
            target_entity_id: value.target_entity_id,
            relation_type: value.relation_type,
            weight: value.weight,
        }
    }
}

impl From<KnowledgeChunkDetail> for KnowledgeChunkDetailDto {
    fn from(value: KnowledgeChunkDetail) -> Self {
        Self {
            id: value.chunk.id,
            assistant_id: value.chunk.assistant_id,
            content: value.chunk.content,
            tags: parse_db_tags(value.chunk.tags.as_ref()),
            source: value.chunk.source,
            created_at: value.chunk.created_at,
            primary_entity_ids: value.primary_entity_ids,
            entities: value.entities.into_iter().map(Into::into).collect(),
            relationships: value.relationships.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<KnowledgeListCursor> for KnowledgeListCursorDto {
    fn from(value: KnowledgeListCursor) -> Self {
        Self {
            created_at: value.created_at,
            id: value.id,
        }
    }
}

impl From<KnowledgeListCursorDto> for KnowledgeListCursor {
    fn from(value: KnowledgeListCursorDto) -> Self {
        Self {
            created_at: value.created_at,
            id: value.id,
        }
    }
}

#[command]
pub async fn list_global_knowledge(
    request: Option<ListGlobalKnowledgeRequest>,
) -> Result<GlobalKnowledgeListResponse, String> {
    let request = request.unwrap_or(ListGlobalKnowledgeRequest {
        query: None,
        assistant_id: None,
        cursor: None,
        limit: None,
    });
    let limit = request
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let repo = get_knowledge_v2_repository();

    let page = repo
        .list_chunks(
            request.assistant_id.as_deref(),
            request.query.as_deref(),
            request.cursor.map(Into::into),
            limit,
        )
        .await?;
    let items = page
        .items
        .into_iter()
        .map(|chunk| KnowledgeChunkListItemDto {
            id: chunk.id,
            assistant_id: chunk.assistant_id,
            preview: build_preview(&chunk.content),
            content: chunk.content,
            tags: parse_db_tags(chunk.tags.as_ref()),
            source: chunk.source,
            created_at: chunk.created_at,
        })
        .collect::<Vec<_>>();

    let assistants = repo.list_assistant_ids().await?;

    Ok(GlobalKnowledgeListResponse {
        items,
        assistants,
        next_cursor: page.next_cursor.map(Into::into),
    })
}

#[command]
pub async fn get_global_knowledge_detail(id: i32) -> Result<KnowledgeChunkDetailDto, String> {
    get_knowledge_v2_repository()
        .get_chunk_detail(id)
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[command]
pub async fn delete_global_knowledge(id: i32) -> Result<DeleteGlobalKnowledgeResponse, String> {
    let summary = get_knowledge_v2_repository()
        .delete_chunk_global(id)
        .await?;
    Ok(DeleteGlobalKnowledgeResponse {
        deleted_chunk_id: id,
        orphan_entity_count: summary.orphan_entity_count,
        orphan_relationship_count: summary.orphan_relationship_count,
    })
}
