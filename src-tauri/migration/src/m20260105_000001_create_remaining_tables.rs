use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create stores table (1:1 with session)
        manager
            .create_table(
                Table::create()
                    .table(Stores::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Stores::SessionId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Stores::Name).string())
                    .col(ColumnDef::new(Stores::Description).string())
                    .col(ColumnDef::new(Stores::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Stores::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        // Create contents table
        manager
            .create_table(
                Table::create()
                    .table(Contents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Contents::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Contents::SessionId).string().not_null())
                    .col(ColumnDef::new(Contents::Filename).string().not_null())
                    .col(ColumnDef::new(Contents::MimeType).string().not_null())
                    .col(ColumnDef::new(Contents::Size).integer().not_null())
                    .col(ColumnDef::new(Contents::LineCount).integer().not_null())
                    .col(ColumnDef::new(Contents::Preview).string().not_null())
                    .col(ColumnDef::new(Contents::UploadedAt).string().not_null())
                    .col(ColumnDef::new(Contents::ChunkCount).integer().not_null())
                    .col(ColumnDef::new(Contents::LastAccessedAt).string().not_null())
                    .col(ColumnDef::new(Contents::Content).string().not_null())
                    .col(ColumnDef::new(Contents::SrcUrl).string())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_contents_stores")
                            .from(Contents::Table, Contents::SessionId)
                            .to(Stores::Table, Stores::SessionId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on contents.session_id
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_contents_session_id")
                    .table(Contents::Table)
                    .col(Contents::SessionId)
                    .to_owned(),
            )
            .await?;

        // Create chunks table
        manager
            .create_table(
                Table::create()
                    .table(Chunks::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Chunks::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Chunks::ContentId).string().not_null())
                    .col(ColumnDef::new(Chunks::ChunkIndex).integer().not_null())
                    .col(ColumnDef::new(Chunks::Text).string().not_null())
                    .col(ColumnDef::new(Chunks::StartLine).integer().not_null())
                    .col(ColumnDef::new(Chunks::EndLine).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_chunks_contents")
                            .from(Chunks::Table, Chunks::ContentId)
                            .to(Contents::Table, Contents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on chunks.content_id
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_chunks_content_id")
                    .table(Chunks::Table)
                    .col(Chunks::ContentId)
                    .to_owned(),
            )
            .await?;

        // Create knowledge table
        manager
            .create_table(
                Table::create()
                    .table(Knowledge::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Knowledge::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Knowledge::AssistantId).string().not_null())
                    .col(ColumnDef::new(Knowledge::Title).string().not_null())
                    .col(ColumnDef::new(Knowledge::Content).string().not_null())
                    .col(ColumnDef::new(Knowledge::Tags).string())
                    .col(
                        ColumnDef::new(Knowledge::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Knowledge::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on knowledge.assistant_id
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_knowledge_assistant")
                    .table(Knowledge::Table)
                    .col(Knowledge::AssistantId)
                    .to_owned(),
            )
            .await?;

        // Create FTS5 virtual table for knowledge (raw SQL)
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts
                USING fts5(title, content, content=knowledge, content_rowid=id)
                "#,
            )
            .await?;

        // Create FTS5 triggers (raw SQL)
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_ai AFTER INSERT ON knowledge BEGIN
                    INSERT INTO knowledge_fts(rowid, title, content)
                    VALUES (new.id, new.title, new.content);
                END
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_ad AFTER DELETE ON knowledge BEGIN
                    INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content)
                    VALUES('delete', old.id, old.title, old.content);
                END
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER IF NOT EXISTS knowledge_au AFTER UPDATE ON knowledge BEGIN
                    INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content)
                    VALUES('delete', old.id, old.title, old.content);
                    INSERT INTO knowledge_fts(rowid, title, content)
                    VALUES (new.id, new.title, new.content);
                END
                "#,
            )
            .await?;

        // Create assistants table (global scope - no session FK)
        manager
            .create_table(
                Table::create()
                    .table(Assistants::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Assistants::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Assistants::Name).string().not_null())
                    .col(ColumnDef::new(Assistants::Config).string().not_null())
                    .col(
                        ColumnDef::new(Assistants::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Assistants::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on assistants.updated_at
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_assistants_updated")
                    .table(Assistants::Table)
                    .col(Assistants::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        // Create playbooks table
        manager
            .create_table(
                Table::create()
                    .table(Playbooks::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Playbooks::Id).string().not_null())
                    .col(ColumnDef::new(Playbooks::SessionId).string().not_null())
                    .col(ColumnDef::new(Playbooks::AssistantId).string().not_null())
                    .col(ColumnDef::new(Playbooks::Goal).string().not_null())
                    .col(ColumnDef::new(Playbooks::InitialCommand).string())
                    .col(ColumnDef::new(Playbooks::Workflow).string().not_null())
                    .col(ColumnDef::new(Playbooks::SuccessCriteria).string())
                    .col(
                        ColumnDef::new(Playbooks::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Playbooks::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    // Composite primary key: id + assistant_id (playbooks are assistant-scoped)
                    .primary_key(
                        Index::create()
                            .col(Playbooks::Id)
                            .col(Playbooks::AssistantId),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes on playbooks
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_session")
                    .table(Playbooks::Table)
                    .col(Playbooks::SessionId)
                    .to_owned(),
            )
            .await?;

        // Create index on assistant_id for quick lookup
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_assistant")
                    .table(Playbooks::Table)
                    .col(Playbooks::AssistantId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playbooks_updated")
                    .table(Playbooks::Table)
                    .col(Playbooks::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        // Create mcp_servers table (global scope)
        manager
            .create_table(
                Table::create()
                    .table(McpServers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(McpServers::Name)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(McpServers::Config).string().not_null())
                    .col(
                        ColumnDef::new(McpServers::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpServers::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order (respecting foreign keys)
        manager
            .drop_table(Table::drop().table(McpServers::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Playbooks::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Assistants::Table).to_owned())
            .await?;

        // Drop FTS5 triggers
        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_au")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_ad")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP TRIGGER IF EXISTS knowledge_ai")
            .await?;

        // Drop FTS5 virtual table
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS knowledge_fts")
            .await?;

        manager
            .drop_table(Table::drop().table(Knowledge::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Chunks::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Contents::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Stores::Table).to_owned())
            .await?;

        Ok(())
    }
}

// Table identifiers
#[derive(DeriveIden)]
enum Stores {
    Table,
    SessionId,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Contents {
    Table,
    Id,
    SessionId,
    Filename,
    MimeType,
    Size,
    LineCount,
    Preview,
    UploadedAt,
    ChunkCount,
    LastAccessedAt,
    Content,
    SrcUrl,
}

#[derive(DeriveIden)]
enum Chunks {
    Table,
    Id,
    ContentId,
    ChunkIndex,
    Text,
    StartLine,
    EndLine,
}

#[derive(DeriveIden)]
enum Knowledge {
    Table,
    Id,
    AssistantId,
    Title,
    Content,
    Tags,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Assistants {
    Table,
    Id,
    Name,
    Config,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Playbooks {
    Table,
    Id,
    SessionId,
    AssistantId,
    Goal,
    InitialCommand,
    Workflow,
    SuccessCriteria,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum McpServers {
    Table,
    Name,
    Config,
    CreatedAt,
    UpdatedAt,
}
