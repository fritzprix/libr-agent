use crate::entity::{knowledge_chunk_entity, knowledge_chunk_v2, knowledge_relationship};
use crate::repositories::DbError;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Statement,
};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
struct GraphContextNode {
    id: i32,
    name: Option<String>,
    #[serde(rename = "type")]
    entity_type: Option<String>,
    description: Option<String>,
    depth: Option<i32>,
}

#[derive(Serialize)]
struct GraphContextEdge {
    source_id: i32,
    target_id: i32,
    #[serde(rename = "type")]
    relation_type: String,
    weight: f32,
}

#[derive(Serialize)]
struct GraphContextChunk {
    id: i32,
    content: String,
    tags: Option<String>,
    source: Option<String>,
    created_at: i64,
    entity_ids: Vec<i32>,
}

#[derive(Serialize)]
struct GraphContextPayload<'a> {
    root_entity: &'a str,
    max_depth: u32,
    nodes: Vec<GraphContextNode>,
    edges: Vec<GraphContextEdge>,
    linked_chunks: Vec<GraphContextChunk>,
}

fn build_missing_entity_response() -> serde_json::Value {
    serde_json::json!({ "error": "Entity not found" })
}

async fn load_graph_nodes(
    db: &DatabaseConnection,
    assistant_id: &str,
    entity_name: &str,
    depth_limit: u32,
) -> Result<(Vec<GraphContextNode>, Vec<i32>), DbError> {
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

    let entity_rows = db
        .query_all(Statement::from_sql_and_values(
            db.get_database_backend(),
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

    let mut nodes = Vec::new();
    let mut entity_ids = Vec::new();
    for row in entity_rows {
        let id: i32 = row.try_get("", "id").map_err(DbError::SeaOrmQueryFailed)?;
        entity_ids.push(id);
        nodes.push(GraphContextNode {
            id,
            name: row.try_get("", "name").ok(),
            entity_type: row.try_get("", "entity_type").ok(),
            description: row.try_get("", "description").ok(),
            depth: row.try_get("", "current_depth").ok(),
        });
    }

    Ok((nodes, entity_ids))
}

async fn load_graph_edges(
    db: &DatabaseConnection,
    assistant_id: &str,
    entity_ids: &[i32],
) -> Result<Vec<GraphContextEdge>, DbError> {
    let relationships = knowledge_relationship::Entity::find()
        .filter(knowledge_relationship::Column::AssistantId.eq(assistant_id))
        .filter(
            Condition::all()
                .add(knowledge_relationship::Column::SourceEntityId.is_in(entity_ids.to_vec()))
                .add(knowledge_relationship::Column::TargetEntityId.is_in(entity_ids.to_vec())),
        )
        .all(db)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    Ok(relationships
        .into_iter()
        .map(|relationship| GraphContextEdge {
            source_id: relationship.source_entity_id,
            target_id: relationship.target_entity_id,
            relation_type: relationship.relation_type,
            weight: relationship.weight,
        })
        .collect())
}

async fn load_linked_chunks(
    db: &DatabaseConnection,
    assistant_id: &str,
    entity_ids: Vec<i32>,
) -> Result<Vec<GraphContextChunk>, DbError> {
    let chunk_links = if entity_ids.is_empty() {
        Vec::new()
    } else {
        knowledge_chunk_entity::Entity::find()
            .filter(knowledge_chunk_entity::Column::EntityId.is_in(entity_ids))
            .all(db)
            .await
            .map_err(DbError::SeaOrmQueryFailed)?
    };

    let linked_chunk_ids = chunk_links
        .iter()
        .map(|link| link.chunk_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let linked_chunks = if linked_chunk_ids.is_empty() {
        Vec::new()
    } else {
        knowledge_chunk_v2::Entity::find()
            .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
            .filter(knowledge_chunk_v2::Column::Id.is_in(linked_chunk_ids))
            .all(db)
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

    Ok(linked_chunks
        .into_iter()
        .map(|chunk| {
            let mut entity_ids = chunk_entity_ids.remove(&chunk.id).unwrap_or_default();
            entity_ids.sort_unstable();

            GraphContextChunk {
                id: chunk.id,
                content: chunk.content,
                tags: chunk.tags,
                source: chunk.source,
                created_at: chunk.created_at,
                entity_ids,
            }
        })
        .collect())
}

pub(super) async fn get_graph_context(
    db: &DatabaseConnection,
    assistant_id: &str,
    entity_name: &str,
    depth: u32,
) -> Result<serde_json::Value, DbError> {
    let depth_limit = depth.min(3);
    let (nodes, entity_ids) = load_graph_nodes(db, assistant_id, entity_name, depth_limit).await?;

    if nodes.is_empty() {
        return Ok(build_missing_entity_response());
    }

    let payload = GraphContextPayload {
        root_entity: entity_name,
        max_depth: depth_limit,
        edges: load_graph_edges(db, assistant_id, &entity_ids).await?,
        linked_chunks: load_linked_chunks(db, assistant_id, entity_ids).await?,
        nodes,
    };

    serde_json::to_value(payload).map_err(|error| DbError::SerializationError(error.to_string()))
}
