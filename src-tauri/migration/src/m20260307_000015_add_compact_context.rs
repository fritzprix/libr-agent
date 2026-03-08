use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CompactContexts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CompactContexts::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CompactContexts::SessionId)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(CompactContexts::FromId).string().not_null())
                    .col(ColumnDef::new(CompactContexts::ToId).string().not_null())
                    .col(ColumnDef::new(CompactContexts::Summary).string().not_null())
                    .col(
                        ColumnDef::new(CompactContexts::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-compact_contexts-session_id")
                            .from(CompactContexts::Table, CompactContexts::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CompactContexts::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CompactContexts {
    #[sea_orm(iden = "compact_contexts")]
    Table,
    Id,
    SessionId,
    FromId,
    ToId,
    Summary,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    Id,
}
