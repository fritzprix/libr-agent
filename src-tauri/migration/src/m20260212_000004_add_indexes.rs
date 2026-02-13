use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Index on messages(session_id, created_at)
        // Optimizes fetching chat history ordered by time
        manager
            .create_index(
                Index::create()
                    .name("idx-messages-session_created")
                    .table(Messages::Table)
                    .col(Messages::SessionId)
                    .col(Messages::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 2. Index on contents(session_id)
        // Optimizes fetching all contents for a session
        manager
            .create_index(
                Index::create()
                    .name("idx-contents-session_id")
                    .table(Contents::Table)
                    .col(Contents::SessionId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 3. Index on chunks(content_id)
        // Optimizes fetching chunks for a specific content (RAG, display)
        manager
            .create_index(
                Index::create()
                    .name("idx-chunks-content_id")
                    .table(Chunks::Table)
                    .col(Chunks::ContentId)
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
                    .name("idx-chunks-content_id")
                    .table(Chunks::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-contents-session_id")
                    .table(Contents::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-messages-session_created")
                    .table(Messages::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Messages {
    #[sea_orm(iden = "messages")]
    Table,
    SessionId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Contents {
    #[sea_orm(iden = "contents")]
    Table,
    SessionId,
}

#[derive(DeriveIden)]
enum Chunks {
    #[sea_orm(iden = "chunks")]
    Table,
    ContentId,
}
