use crate::helpers::column_exists;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if column_exists(manager, "scheduled_tasks", "schedule_timezone").await? {
            return Ok(());
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ScheduledTasks::Table)
                    .add_column(
                        ColumnDef::new(ScheduledTasks::ScheduleTimezone)
                            .string()
                            .not_null()
                            .default("utc"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !column_exists(manager, "scheduled_tasks", "schedule_timezone").await? {
            return Ok(());
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ScheduledTasks::Table)
                    .drop_column(ScheduledTasks::ScheduleTimezone)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ScheduledTasks {
    #[sea_orm(iden = "scheduled_tasks")]
    Table,
    ScheduleTimezone,
}
