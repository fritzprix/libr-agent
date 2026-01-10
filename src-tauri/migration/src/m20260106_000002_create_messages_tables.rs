use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create messages table
        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Messages::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Messages::SessionId).string().not_null())
                    .col(ColumnDef::new(Messages::Role).string().not_null())
                    .col(ColumnDef::new(Messages::Content).text().not_null())
                    .col(ColumnDef::new(Messages::ToolCalls).string())
                    .col(ColumnDef::new(Messages::ToolCallId).string())
                    .col(ColumnDef::new(Messages::IsStreaming).integer())
                    .col(ColumnDef::new(Messages::Thinking).string())
                    .col(ColumnDef::new(Messages::ThinkingSignature).string())
                    .col(ColumnDef::new(Messages::AssistantId).string())
                    .col(ColumnDef::new(Messages::Attachments).string())
                    .col(ColumnDef::new(Messages::ToolUse).string())
                    .col(ColumnDef::new(Messages::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Messages::UpdatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Messages::Source).string())
                    .col(ColumnDef::new(Messages::Error).string())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Messages::Table, Messages::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for messages
        manager
            .create_index(
                Index::create()
                    .name("idx_messages_session_id")
                    .table(Messages::Table)
                    .col(Messages::SessionId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_messages_session_created")
                    .table(Messages::Table)
                    .col(Messages::SessionId)
                    .col(Messages::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Create message_index_meta table
        manager
            .create_table(
                Table::create()
                    .table(MessageIndexMeta::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MessageIndexMeta::SessionId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MessageIndexMeta::IndexPath).string())
                    .col(
                        ColumnDef::new(MessageIndexMeta::LastIndexedAt)
                            .big_integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MessageIndexMeta::DocCount)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MessageIndexMeta::IndexVersion)
                            .integer()
                            .default(1),
                    )
                    .col(ColumnDef::new(MessageIndexMeta::LastRebuildDurationMs).big_integer())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MessageIndexMeta::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Messages::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Sessions {
    Table,
    Id,
}

#[derive(Iden)]
enum Messages {
    Table,
    Id,
    SessionId,
    Role,
    Content,
    ToolCalls,
    ToolCallId,
    IsStreaming,
    Thinking,
    ThinkingSignature,
    AssistantId,
    Attachments,
    ToolUse,
    CreatedAt,
    UpdatedAt,
    Source,
    Error,
}

#[derive(Iden)]
enum MessageIndexMeta {
    Table,
    SessionId,
    IndexPath,
    LastIndexedAt,
    DocCount,
    IndexVersion,
    LastRebuildDurationMs,
}
