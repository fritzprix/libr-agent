use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op: stores.session_id is already the PRIMARY KEY, so SQLite
        // already maintains an implicit index for session lookups.
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op: up does not create anything, so there is nothing to drop.
        Ok(())
    }
}
