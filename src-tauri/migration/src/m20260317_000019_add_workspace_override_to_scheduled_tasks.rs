use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ScheduledTasks::Table)
                    .add_column(
                        ColumnDef::new(ScheduledTasks::WorkspaceOverride)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ScheduledTasks::Table)
                    .drop_column(ScheduledTasks::WorkspaceOverride)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ScheduledTasks {
    #[sea_orm(iden = "scheduled_tasks")]
    Table,
    WorkspaceOverride,
}
