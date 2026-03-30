use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Index on stores(session_id)
        // Optimizes fetching stores for a specific session
        manager
            .create_index(
                Index::create()
                    .name("idx-stores-session_id")
                    .table(Stores::Table)
                    .col(Stores::SessionId)
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
                    .name("idx-stores-session_id")
                    .table(Stores::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Stores {
    #[sea_orm(iden = "stores")]
    Table,
    SessionId,
}
