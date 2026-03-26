use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1. Create Knowledge Chunks Table (Metadata & Full text)
        manager
            .create_table(
                Table::create()
                    .table(KnowledgeChunksV2::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(KnowledgeChunksV2::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(KnowledgeChunksV2::AssistantId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(KnowledgeChunksV2::Content).text().not_null())
                    .col(ColumnDef::new(KnowledgeChunksV2::Tags).text()) // JSON array
                    .col(ColumnDef::new(KnowledgeChunksV2::Source).string())
                    .col(
                        ColumnDef::new(KnowledgeChunksV2::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Create FTS5 virtual table for knowledge_chunks_v2
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_chunks_fts USING fts5(content, tags, source, content='knowledge_chunks_v2', content_rowid='id');".to_owned(),
        ))
        .await?;

        // Trigger for FTS INSERT
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TRIGGER IF NOT EXISTS knowledge_chunks_fts_ai AFTER INSERT ON knowledge_chunks_v2 BEGIN
              INSERT INTO knowledge_chunks_fts(rowid, content, tags, source) VALUES (new.id, new.content, new.tags, new.source);
            END;".to_owned(),
        )).await?;

        // Trigger for FTS UPDATE
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TRIGGER IF NOT EXISTS knowledge_chunks_fts_au AFTER UPDATE ON knowledge_chunks_v2 BEGIN
              UPDATE knowledge_chunks_fts SET content = new.content, tags = new.tags, source = new.source WHERE rowid = new.id;
            END;".to_owned(),
        )).await?;

        // Trigger for FTS DELETE
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TRIGGER IF NOT EXISTS knowledge_chunks_fts_ad AFTER DELETE ON knowledge_chunks_v2 BEGIN
              DELETE FROM knowledge_chunks_fts WHERE rowid = old.id;
            END;".to_owned(),
        )).await?;

        // 3. Create sqlite-vec virtual table for Embeddings
        // We use 384 dimensions for all-MiniLM-L6-v2 or bge-small-en-v1.5
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_vectors USING vec0(embedding float[384]);".to_owned(),
        ))
        .await?;

        // 4. Create Graph Entities Table
        manager
            .create_table(
                Table::create()
                    .table(KnowledgeEntities::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(KnowledgeEntities::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(KnowledgeEntities::AssistantId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(KnowledgeEntities::Name).string().not_null())
                    .col(ColumnDef::new(KnowledgeEntities::EntityType).string()) // e.g., "Person", "Project"
                    .col(ColumnDef::new(KnowledgeEntities::Description).text())
                    .to_owned(),
            )
            .await?;

        // Create a unique index to prevent duplicate entities for the same assistant
        manager
            .create_index(
                Index::create()
                    .name("idx_knowledge_entities_unique")
                    .table(KnowledgeEntities::Table)
                    .col(KnowledgeEntities::AssistantId)
                    .col(KnowledgeEntities::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 5. Create Graph Relationships Table (Edges)
        manager
            .create_table(
                Table::create()
                    .table(KnowledgeRelationships::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(KnowledgeRelationships::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(KnowledgeRelationships::AssistantId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(KnowledgeRelationships::SourceEntityId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(KnowledgeRelationships::TargetEntityId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(KnowledgeRelationships::RelationType)
                            .string()
                            .not_null(),
                    ) // e.g., "WORKS_ON"
                    .col(
                        ColumnDef::new(KnowledgeRelationships::Weight)
                            .float()
                            .default(1.0),
                    )
                    .to_owned(),
            )
            .await?;

        // 6. Create mapping between Entities and Chunks (Citations/Evidence)
        manager
            .create_table(
                Table::create()
                    .table(KnowledgeChunkEntities::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(KnowledgeChunkEntities::ChunkId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(KnowledgeChunkEntities::EntityId)
                            .integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(KnowledgeChunkEntities::ChunkId)
                            .col(KnowledgeChunkEntities::EntityId),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        manager
            .drop_table(
                Table::drop()
                    .table(KnowledgeChunkEntities::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(KnowledgeRelationships::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(KnowledgeEntities::Table).to_owned())
            .await?;

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TABLE IF EXISTS knowledge_vectors;".to_owned(),
        ))
        .await?;

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TRIGGER IF EXISTS knowledge_chunks_fts_ad;".to_owned(),
        ))
        .await?;
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TRIGGER IF EXISTS knowledge_chunks_fts_au;".to_owned(),
        ))
        .await?;
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TRIGGER IF EXISTS knowledge_chunks_fts_ai;".to_owned(),
        ))
        .await?;

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "DROP TABLE IF EXISTS knowledge_chunks_fts;".to_owned(),
        ))
        .await?;

        manager
            .drop_table(Table::drop().table(KnowledgeChunksV2::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum KnowledgeChunksV2 {
    Table,
    Id,
    AssistantId,
    Content,
    Tags,
    Source,
    CreatedAt,
}

#[derive(DeriveIden)]
enum KnowledgeEntities {
    Table,
    Id,
    AssistantId,
    Name,
    EntityType,
    Description,
}

#[derive(DeriveIden)]
enum KnowledgeRelationships {
    Table,
    Id,
    AssistantId,
    SourceEntityId,
    TargetEntityId,
    RelationType,
    Weight,
}

#[derive(DeriveIden)]
enum KnowledgeChunkEntities {
    Table,
    ChunkId,
    EntityId,
}
