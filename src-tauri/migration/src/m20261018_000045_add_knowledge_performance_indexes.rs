use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Semantic KNN and keyword searches filter chunks by assistant_id.
        manager
            .create_index(
                Index::create()
                    .name("idx-knowledge_chunks_v2-assistant_id")
                    .table(KnowledgeChunksV2::Table)
                    .col(KnowledgeChunksV2::AssistantId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Global graph and entity listing check `EXISTS (SELECT 1 FROM knowledge_chunk_entities WHERE entity_id = e.id)`.
        // Composite PK is (chunk_id, entity_id), so secondary index on entity_id prevents full table scans.
        manager
            .create_index(
                Index::create()
                    .name("idx-knowledge_chunk_entities-entity_id")
                    .table(KnowledgeChunkEntities::Table)
                    .col(KnowledgeChunkEntities::EntityId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-knowledge_chunk_entities-entity_id")
                    .table(KnowledgeChunkEntities::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-knowledge_chunks_v2-assistant_id")
                    .table(KnowledgeChunksV2::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum KnowledgeChunksV2 {
    #[sea_orm(iden = "knowledge_chunks_v2")]
    Table,
    AssistantId,
}

#[derive(DeriveIden)]
enum KnowledgeChunkEntities {
    #[sea_orm(iden = "knowledge_chunk_entities")]
    Table,
    EntityId,
}
