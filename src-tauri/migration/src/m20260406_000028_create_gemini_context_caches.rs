use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Compatibility migration:
        // This version was briefly shipped in the migration chain during development.
        // It must remain registered so databases that already recorded migration #28
        // keep a stable migration count/version history. The table itself is not
        // used by the current runtime, so this migration intentionally performs
        // no schema change for fresh databases.
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
