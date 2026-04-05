use crate::helpers::column_exists;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !column_exists(manager, "scheduled_tasks", "group_id").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .add_column(ColumnDef::new(ScheduledTasks::GroupId).string().null())
                        .to_owned(),
                )
                .await?;
        }

        if !column_exists(manager, "scheduled_tasks", "group_name").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .add_column(ColumnDef::new(ScheduledTasks::GroupName).string().null())
                        .to_owned(),
                )
                .await?;
        }

        if !column_exists(manager, "scheduled_tasks", "created_by_session_id").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .add_column(
                            ColumnDef::new(ScheduledTasks::CreatedBySessionId)
                                .string()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if column_exists(manager, "scheduled_tasks", "created_by_session_id").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .drop_column(ScheduledTasks::CreatedBySessionId)
                        .to_owned(),
                )
                .await?;
        }

        if column_exists(manager, "scheduled_tasks", "group_name").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .drop_column(ScheduledTasks::GroupName)
                        .to_owned(),
                )
                .await?;
        }

        if column_exists(manager, "scheduled_tasks", "group_id").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .drop_column(ScheduledTasks::GroupId)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ScheduledTasks {
    #[sea_orm(iden = "scheduled_tasks")]
    Table,
    GroupId,
    GroupName,
    CreatedBySessionId,
}
