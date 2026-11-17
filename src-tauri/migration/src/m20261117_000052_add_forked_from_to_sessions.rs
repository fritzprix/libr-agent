use crate::helpers::column_exists;
use sea_orm_migration::prelude::*;

/// Session write-back fork origin (`S8`, #2860).
///
/// When a `--session` headless run cannot merge into its original session
/// (the profile changed underneath it, or a live GUI owns it), the run's
/// history is written back as a new session row whose `forked_from` records
/// the original session id. Merged and freshly created sessions keep it NULL.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !column_exists(manager, "sessions", "forked_from").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Sessions::Table)
                        .add_column(ColumnDef::new(Sessions::ForkedFrom).text().null())
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if column_exists(manager, "sessions", "forked_from").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Sessions::Table)
                        .drop_column(Sessions::ForkedFrom)
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
    ForkedFrom,
}
