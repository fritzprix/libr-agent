use crate::helpers::column_exists;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Note: rolling back past m20260613 (add unsafe_mode) then re-applying leaves scheduled_tasks
// without unsafe_mode until m37 runs again. This is acceptable — full down chains are rare.

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        normalize_execution_mode_for_table(manager, "sessions", Sessions::Table).await?;
        normalize_execution_mode_for_table(manager, "scheduled_tasks", ScheduledTasks::Table)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        restore_execution_mode_flags_for_table(manager, "sessions", Sessions::Table).await?;
        restore_execution_mode_flags_for_table(manager, "scheduled_tasks", ScheduledTasks::Table)
            .await?;
        Ok(())
    }
}

async fn normalize_execution_mode_for_table<T>(
    manager: &SchemaManager<'_>,
    table_name: &str,
    table: T,
) -> Result<(), DbErr>
where
    T: IntoIden + Clone + 'static,
{
    if !column_exists(manager, table_name, "execution_mode").await? {
        manager
            .alter_table(
                Table::alter()
                    .table(table.clone())
                    .add_column(
                        ColumnDef::new(ExecutionModeCol::ExecutionMode)
                            .string()
                            .not_null()
                            .default("normal"),
                    )
                    .to_owned(),
            )
            .await?;
    }

    if column_exists(manager, table_name, "yolo_mode").await?
        && column_exists(manager, table_name, "unsafe_mode").await?
    {
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                format!(
                    "UPDATE {table_name} SET execution_mode = CASE \
                     WHEN unsafe_mode = 1 THEN 'unsafe' \
                     WHEN yolo_mode = 1 THEN 'yolo' \
                     ELSE 'normal' END"
                ),
            ))
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(table.clone())
                    .drop_column(LegacyModeCol::YoloMode)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(table)
                    .drop_column(LegacyModeCol::UnsafeMode)
                    .to_owned(),
            )
            .await?;
    }

    Ok(())
}

async fn restore_execution_mode_flags_for_table<T>(
    manager: &SchemaManager<'_>,
    table_name: &str,
    table: T,
) -> Result<(), DbErr>
where
    T: IntoIden + Clone + 'static,
{
    if !column_exists(manager, table_name, "execution_mode").await? {
        return Ok(());
    }

    if !column_exists(manager, table_name, "yolo_mode").await? {
        manager
            .alter_table(
                Table::alter()
                    .table(table.clone())
                    .add_column(
                        ColumnDef::new(LegacyModeCol::YoloMode)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;
    }

    if !column_exists(manager, table_name, "unsafe_mode").await? {
        manager
            .alter_table(
                Table::alter()
                    .table(table.clone())
                    .add_column(
                        ColumnDef::new(LegacyModeCol::UnsafeMode)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;
    }

    manager
        .get_connection()
        .execute(Statement::from_string(
            manager.get_database_backend(),
            format!(
                "UPDATE {table_name} SET \
                 yolo_mode = CASE WHEN execution_mode = 'yolo' THEN 1 ELSE 0 END, \
                 unsafe_mode = CASE WHEN execution_mode = 'unsafe' THEN 1 ELSE 0 END"
            ),
        ))
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(table)
                .drop_column(ExecutionModeCol::ExecutionMode)
                .to_owned(),
        )
        .await?;

    Ok(())
}

#[derive(DeriveIden, Clone, Copy)]
enum Sessions {
    #[sea_orm(iden = "sessions")]
    Table,
}

#[derive(DeriveIden, Clone, Copy)]
enum ScheduledTasks {
    #[sea_orm(iden = "scheduled_tasks")]
    Table,
}

#[derive(DeriveIden)]
enum ExecutionModeCol {
    ExecutionMode,
}

#[derive(DeriveIden)]
enum LegacyModeCol {
    YoloMode,
    UnsafeMode,
}
