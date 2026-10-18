use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // list_sessions filters on status and orders by (updated_at DESC, id DESC).
        manager
            .create_index(
                Index::create()
                    .name("idx-sessions-status_updated_id")
                    .table(Sessions::Table)
                    .col(Sessions::Status)
                    .col(Sessions::UpdatedAt)
                    .col(Sessions::Id)
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
                    .name("idx-sessions-status_updated_id")
                    .table(Sessions::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    Status,
    UpdatedAt,
    Id,
}
