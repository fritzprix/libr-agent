use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create Message Index Meta table if it doesn't exist
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
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-message_index_meta-session_id")
                            .from(MessageIndexMeta::Table, MessageIndexMeta::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(MessageIndexMeta::IndexPath).string())
                    .col(
                        ColumnDef::new(MessageIndexMeta::LastIndexedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MessageIndexMeta::DocCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MessageIndexMeta::IndexVersion)
                            .integer()
                            .not_null(),
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

        Ok(())
    }
}

#[derive(DeriveIden)]
enum MessageIndexMeta {
    #[sea_orm(iden = "message_index_meta")]
    Table,
    SessionId,
    IndexPath,
    LastIndexedAt,
    DocCount,
    IndexVersion,
    LastRebuildDurationMs,
}

#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
}
