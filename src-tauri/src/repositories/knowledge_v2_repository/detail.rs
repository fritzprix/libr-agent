use crate::entity::{
    knowledge_chunk_entity, knowledge_chunk_v2, knowledge_entity, knowledge_relationship,
};
use crate::repositories::DbError;
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashSet;

use super::contracts::{KnowledgeChunkDetail, KnowledgeGraphEntity, KnowledgeGraphRelationship};

fn sort_entities(entities: &mut [KnowledgeGraphEntity]) {
    entities.sort_by(|left, right| {
        right
            .is_primary
            .cmp(&left.is_primary)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn sort_relationships(relationships: &mut [KnowledgeGraphRelationship]) {
    relationships.sort_by(|left, right| {
        left.relation_type
            .cmp(&right.relation_type)
            .then_with(|| left.source_entity_id.cmp(&right.source_entity_id))
            .then_with(|| left.target_entity_id.cmp(&right.target_entity_id))
    });
}

pub(super) async fn get_chunk_detail(
    db: &DatabaseConnection,
    id: i32,
) -> Result<KnowledgeChunkDetail, DbError> {
    let chunk = knowledge_chunk_v2::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?
        .ok_or_else(|| DbError::NotFound(format!("Chunk {} not found", id)))?;

    let chunk_links = knowledge_chunk_entity::Entity::find()
        .filter(knowledge_chunk_entity::Column::ChunkId.eq(id))
        .all(db)
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
        .all(db)
        .await
        .map_err(DbError::SeaOrmQueryFailed)?;

    let mut all_entity_ids = primary_entity_ids.iter().copied().collect::<HashSet<_>>();
    for relationship in &relationship_rows {
        all_entity_ids.insert(relationship.source_entity_id);
        all_entity_ids.insert(relationship.target_entity_id);
    }

    let entity_rows = knowledge_entity::Entity::find()
        .filter(knowledge_entity::Column::Id.is_in(all_entity_ids.into_iter().collect::<Vec<_>>()))
        .all(db)
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
    sort_entities(&mut entities);

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
    sort_relationships(&mut relationships);

    Ok(KnowledgeChunkDetail {
        chunk,
        primary_entity_ids,
        entities,
        relationships,
    })
}
