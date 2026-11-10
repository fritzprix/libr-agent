use crate::helpers::column_exists;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !column_exists(manager, "sessions", "status_reason").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Sessions::Table)
                        .add_column(ColumnDef::new(Sessions::StatusReason).text().null())
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if column_exists(manager, "sessions", "status_reason").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Sessions::Table)
                        .drop_column(Sessions::StatusReason)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
    StatusReason,
}
