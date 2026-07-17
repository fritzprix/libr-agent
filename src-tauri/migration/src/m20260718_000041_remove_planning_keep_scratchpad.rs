use crate::helpers::column_exists;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create scratchpad table if not exists
        manager
            .create_table(
                Table::create()
                    .table(Scratchpad::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Scratchpad::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Scratchpad::SessionId).string().not_null())
                    .col(ColumnDef::new(Scratchpad::Content).string().not_null())
                    .col(ColumnDef::new(Scratchpad::Title).string())
                    .col(ColumnDef::new(Scratchpad::Source).string())
                    .col(ColumnDef::new(Scratchpad::Tags).string())
                    .col(
                        ColumnDef::new(Scratchpad::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Scratchpad::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. If planning_scratchpad exists, copy data to scratchpad and drop it
        if manager.has_table("planning_scratchpad").await? {
            let db = manager.get_connection();
            db.execute_unprepared(
                "INSERT INTO scratchpad (id, session_id, content, title, source, tags, created_at, updated_at) \
                 SELECT id, session_id, content, title, source, tags, created_at, updated_at \
                 FROM planning_scratchpad"
            )
            .await?;

            manager
                .drop_table(Table::drop().table(PlanningScratchpad::Table).to_owned())
                .await?;
        }

        // 3. Drop planning_goals if exists
        if manager.has_table("planning_goals").await? {
            manager
                .drop_table(Table::drop().table(PlanningGoals::Table).to_owned())
                .await?;
        }

        // 4. Drop planning_todos if exists
        if manager.has_table("planning_todos").await? {
            manager
                .drop_table(Table::drop().table(PlanningTodos::Table).to_owned())
                .await?;
        }

        // 5. Drop reset_planning_state column from scheduled_tasks if exists
        if column_exists(manager, "scheduled_tasks", "reset_planning_state").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .drop_column(ScheduledTasks::ResetPlanningState)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Recreate tables to restore original state
        if !manager.has_table("planning_scratchpad").await? {
            manager
                .create_table(
                    Table::create()
                        .table(PlanningScratchpad::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(PlanningScratchpad::Id)
                                .integer()
                                .not_null()
                                .auto_increment()
                                .primary_key(),
                        )
                        .col(
                            ColumnDef::new(PlanningScratchpad::SessionId)
                                .string()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PlanningScratchpad::Content)
                                .string()
                                .not_null(),
                        )
                        .col(ColumnDef::new(PlanningScratchpad::Title).string())
                        .col(ColumnDef::new(PlanningScratchpad::Source).string())
                        .col(ColumnDef::new(PlanningScratchpad::Tags).string())
                        .col(
                            ColumnDef::new(PlanningScratchpad::CreatedAt)
                                .big_integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PlanningScratchpad::UpdatedAt)
                                .big_integer()
                                .not_null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Copy data back if scratchpad exists
            if manager.has_table("scratchpad").await? {
                let db = manager.get_connection();
                db.execute_unprepared(
                    "INSERT INTO planning_scratchpad (id, session_id, content, title, source, tags, created_at, updated_at) \
                     SELECT id, session_id, content, title, source, tags, created_at, updated_at \
                     FROM scratchpad"
                )
                .await?;
            }
        }

        if manager.has_table("scratchpad").await? {
            manager
                .drop_table(Table::drop().table(Scratchpad::Table).to_owned())
                .await?;
        }

        if !manager.has_table("planning_goals").await? {
            manager
                .create_table(
                    Table::create()
                        .table(PlanningGoals::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(PlanningGoals::Id)
                                .integer()
                                .not_null()
                                .auto_increment()
                                .primary_key(),
                        )
                        .col(ColumnDef::new(PlanningGoals::SessionId).string().not_null())
                        .col(ColumnDef::new(PlanningGoals::GoalText).string().not_null())
                        .col(ColumnDef::new(PlanningGoals::Status).string().not_null())
                        .col(
                            ColumnDef::new(PlanningGoals::CreatedAt)
                                .big_integer()
                                .not_null(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        if !manager.has_table("planning_todos").await? {
            manager
                .create_table(
                    Table::create()
                        .table(PlanningTodos::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(PlanningTodos::Id)
                                .integer()
                                .not_null()
                                .auto_increment()
                                .primary_key(),
                        )
                        .col(ColumnDef::new(PlanningTodos::SessionId).string().not_null())
                        .col(ColumnDef::new(PlanningTodos::Content).string().not_null())
                        .col(ColumnDef::new(PlanningTodos::Description).string())
                        .col(ColumnDef::new(PlanningTodos::Priority).string().not_null())
                        .col(
                            ColumnDef::new(PlanningTodos::IsChecked)
                                .boolean()
                                .not_null()
                                .default(0),
                        )
                        .col(ColumnDef::new(PlanningTodos::Status).string().not_null())
                        .col(
                            ColumnDef::new(PlanningTodos::CreatedAt)
                                .big_integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PlanningTodos::UpdatedAt)
                                .big_integer()
                                .not_null(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        if !column_exists(manager, "scheduled_tasks", "reset_planning_state").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScheduledTasks::Table)
                        .add_column(
                            ColumnDef::new(ScheduledTasks::ResetPlanningState)
                                .boolean()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Scratchpad {
    #[sea_orm(iden = "scratchpad")]
    Table,
    Id,
    SessionId,
    Content,
    Title,
    Source,
    Tags,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PlanningScratchpad {
    #[sea_orm(iden = "planning_scratchpad")]
    Table,
    Id,
    SessionId,
    Content,
    Title,
    Source,
    Tags,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PlanningGoals {
    #[sea_orm(iden = "planning_goals")]
    Table,
    Id,
    SessionId,
    GoalText,
    Status,
    CreatedAt,
}

#[derive(DeriveIden)]
enum PlanningTodos {
    #[sea_orm(iden = "planning_todos")]
    Table,
    Id,
    SessionId,
    Content,
    Description,
    Priority,
    IsChecked,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ScheduledTasks {
    #[sea_orm(iden = "scheduled_tasks")]
    Table,
    ResetPlanningState,
}
